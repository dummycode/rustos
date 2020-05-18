use core::fmt;
use core::time::Duration;

use shim::io;
use shim::ioerr;
use shim::const_assert_size;

use volatile::prelude::*;
use volatile::{ReadVolatile, Reserved, Volatile};

use crate::common::IO_BASE;
use crate::gpio::{Function, Gpio};
use crate::timer;

/// The base address for the `MU` registers.
const MU_REG_BASE: usize = IO_BASE + 0x215040;

/// The `AUXENB` register from page 9 of the BCM2837 documentation.
const AUX_ENABLES: *mut Volatile<u8> = (IO_BASE + 0x215004) as *mut Volatile<u8>;

/// Enum representing bit fields of the `AUX_MU_LSR_REG` register.
#[repr(u8)]
enum LsrStatus {
  DataReady = 1,
  TxAvailable = 1 << 5,
}

#[repr(C)]
#[allow(non_snake_case)]
struct Registers {
  // FIXME: Declare the "MU" registers from page 8.
  io: Volatile<u8>,
  _reserved0: [Reserved<u8>; 3],
  ier: Volatile<u8>,
  _reserved1: [Reserved<u8>; 3],
  iir: Volatile<u8>,
  _reserved2: [Reserved<u8>; 3],
  lcr: Volatile<u8>,
  _reserved3: [Reserved<u8>; 3],
  mcr: Volatile<u8>,
  _reserved4: [Reserved<u8>; 3],
  lsr: ReadVolatile<u8>,
  _reserved5: [Reserved<u8>; 3],
  msr: ReadVolatile<u8>,
  _reserved6: [Reserved<u8>; 3],
  scratch: Volatile<u8>,
  _reserved7: [Reserved<u8>; 3],
  cntl: Volatile<u8>,
  _reserved8: [Reserved<u8>; 3],
  stat: ReadVolatile<u32>,
  baud: Volatile<u16>,
}

const_assert_size!(Registers, 0x7E21506C - 0x7E215040);

/// The Raspberry Pi's "mini UART".
pub struct MiniUart {
  registers: &'static mut Registers,
  timeout: Option<Duration>,
}

impl MiniUart {
  /// Initializes the mini UART by enabling it as an auxiliary peripheral,
  /// setting the data size to 8 bits, setting the BAUD rate to ~115200 (baud
  /// divider of 270), setting GPIO pins 14 and 15 to alternative function 5
  /// (TXD1/RDXD1), and finally enabling the UART transmitter and receiver.
  ///
  /// By default, reads will never time out. To set a read timeout, use
  /// `set_read_timeout()`.
  pub fn new() -> MiniUart {
    let registers = unsafe {
      // Enable the mini UART as an auxiliary device.
      (*AUX_ENABLES).or_mask(1);
      &mut *(MU_REG_BASE as *mut Registers)
    };

    // Set GPIO pins to alt function 5
    Gpio::new(14).into_alt(Function::Alt5);
    Gpio::new(15).into_alt(Function::Alt5);

    // Set data size to 8 bits
    registers.lcr.or_mask(0b11);

    // Set baud rate
    registers.baud.write(270);

    // Enable transmitter and receiver
    registers.cntl.or_mask(0b11);

    // Flush the queues
    registers.iir.or_mask(0b110);

    return MiniUart {
      registers: registers,
      timeout: None,
    }
  }

  /// Set the read timeout to `t` duration.
  pub fn set_read_timeout(&mut self, t: Duration) {
    self.timeout = Some(t);
  }

  /// Write the byte `byte`. This method blocks until there is space available
  /// in the output FIFO.
  pub fn write_byte(&mut self, byte: u8) {
    while !self.registers.lsr.has_mask(1 << 5 as u8) {}

    self.registers.io.write(byte);
  }

  /// Returns `true` if there is at least one byte ready to be read. If this
  /// method returns `true`, a subsequent call to `read_byte` is guaranteed to
  /// return immediately. This method does not block.
  pub fn has_byte(&self) -> bool {
    return self.registers.lsr.has_mask(1 as u8);
  }

  /// Blocks until there is a byte ready to read. If a read timeout is set,
  /// this method blocks for at most that amount of time. Otherwise, this
  /// method blocks indefinitely until there is a byte to read.
  ///
  /// Returns `Ok(())` if a byte is ready to read. Returns `Err(())` if the
  /// timeout expired while waiting for a byte to be ready. If this method
  /// returns `Ok(())`, a subsequent call to `read_byte` is guaranteed to
  /// return immediately.
  pub fn wait_for_byte(&self) -> Result<(), ()> {
    let start = timer::current_time();

    while !self.has_byte() {
      match self.timeout {
        Some(timeout) => {
          if timer::current_time() - start > timeout {
            return Err(());
          }
        },
        None => {}
      }
    }
    return Ok(());
  }

  /// Reads a byte. Blocks indefinitely until a byte is ready to be read.
  pub fn read_byte(&mut self) -> u8 {
    while !self.has_byte() {}

    return self.registers.io.read();
  }

  /// Flush the transmit queue
  pub fn flush(&mut self) -> Result<(), ()> {
    self.registers.iir.or_mask(0b110);
    return Ok(());
  }
}

// FIXME: Implement `fmt::Write` for `MiniUart`. A b'\r' byte should be written
// before writing any b'\n' byte.
impl fmt::Write for MiniUart {
  fn write_str(&mut self, s: &str) -> Result<(), fmt::Error> {
    for c in s.bytes() {
      if c == b'\n' {
        self.write_byte(b'\r');
      }
      self.write_byte(c);
    }
    return Ok(());
  }
}

mod uart_io {
  use super::io;
  use super::MiniUart;
  use volatile::prelude::*;

  // FIXME: Implement `io::Read` and `io::Write` for `MiniUart`.
  //
  // The `io::Read::read()` implementation must respect the read timeout by
  // waiting at most that time for the _first byte_. It should not wait for
  // any additional bytes but _should_ read as many bytes as possible. If the
  // read times out, an error of kind `TimedOut` should be returned.
  //
  // The `io::Write::write()` method must write all of the requested bytes
  // before returning.

  impl io::Read for MiniUart {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
      if buf.len() == 0 {
        return Ok(0);
      }

      let result = self.wait_for_byte();

      match result {
        Ok(()) => {
          let mut i: usize = 0;

          while i < buf.len() && self.has_byte() {
            buf[i] = self.read_byte();
            i += 1;
          }
          return Ok(i);
        },
        Err(()) => {
          return Err(io::Error::new(io::ErrorKind::TimedOut, "Mini uart timed out"));
        }
      }
    }
  }

  impl io::Write for MiniUart {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
      let mut i: usize = 0;

      while i < buf.len() {
        let byte = buf[i];
        if byte == b'\r' {
          self.write_byte(b'\n');
          i += 1;
        }
        self.write_byte(byte);
        i += 1;
      }
      return Ok(i);
    }

    fn flush(&mut self) -> io::Result<()> {
      self.flush();

      return Ok(());
    }
  }
}
