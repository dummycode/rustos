use core::time::Duration;
use shim::io;
use shim::ioerr;
use pi::timer;

use fat32::traits::BlockDevice;

extern "C" {
    /// A global representing the last SD controller error that occured.
    static sd_err: i64;

    /// Initializes the SD card controller.
    ///
    /// Returns 0 if initialization is successful. If initialization fails,
    /// returns -1 if a timeout occured, or -2 if an error sending commands to
    /// the SD controller occured.
    fn sd_init() -> i32;

    /// Reads sector `n` (512 bytes) from the SD card and writes it to `buffer`.
    /// It is undefined behavior if `buffer` does not point to at least 512
    /// bytes of memory. Also, the caller of this function should make sure that
    /// `buffer` is at least 4-byte aligned.
    ///
    /// On success, returns the number of bytes read: a positive number.
    ///
    /// On error, returns 0. The true error code is stored in the `sd_err`
    /// global. `sd_err` will be set to -1 if a timeout occured or -2 if an
    /// error sending commands to the SD controller occured. Other error codes
    /// are also possible but defined only as being less than zero.
    fn sd_readsector(n: i32, buffer: *mut u8) -> i32;
}

// FIXME: Define a `#[no_mangle]` `wait_micros` function for use by `libsd`.
// The `wait_micros` C signature is: `void wait_micros(unsigned int);`
#[no_mangle]
fn wait_micros(micros: u32) {
    let micros = micros * 100;
    let duration = Duration::from_micros(micros.into());
    timer::spin_sleep(duration);
}

/// A handle to an SD card controller.
#[derive(Debug)]
pub struct Sd;

impl Sd {
    /// Initializes the SD card controller and returns a handle to it.
    /// The caller should assure that the method is invoked only once during the
    /// kernel initialization. We can enforce the requirement in safe Rust code
    /// with atomic memory access, but we can't use it yet since we haven't
    /// written the memory management unit (MMU).
    pub unsafe fn new() -> Result<Sd, io::Error> {
        let res = unsafe { sd_init() };

        match res {
            0 => Ok(Sd),
            -1 => Err(io::Error::new(io::ErrorKind::Other, "Timeout occurred")),
            -2 => Err(io::Error::new(io::ErrorKind::Other, "Error sending commands to sd controller")),
            _ => panic!("Yeah we fucked up"),
        }
    }
}

impl BlockDevice for Sd {
    /// Reads sector `n` from the SD card into `buf`. On success, the number of
    /// bytes read is returned.
    ///
    /// # Errors
    ///
    /// An I/O error of kind `InvalidInput` is returned if `buf.len() < 512` or
    /// `n > 2^31 - 1` (the maximum value for an `i32`).
    ///
    /// An error of kind `TimedOut` is returned if a timeout occurs while
    /// reading from the SD card.
    ///
    /// An error of kind `Other` is returned for all other errors.
    fn read_sector(&mut self, n: u64, buf: &mut [u8]) -> io::Result<usize> {
        assert_eq!(buf.len() % 4, 0);

        if buf.len() < 512 || n > 1<<31 - 1 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid size of buffer"));
        }

        let size = unsafe { sd_readsector(n as i32, &mut buf[0] as *mut u8) };

        if size == 0 {
            match unsafe { sd_err } {
                -1 => return Err(io::Error::new(io::ErrorKind::TimedOut, "Sd card timed out")),
                -2 => return Err(io::Error::new(io::ErrorKind::Other, "Error sending commands")),
                _ => return Err(io::Error::new(io::ErrorKind::Other, "Unknown error occurred")),
            }
        }

        return Ok(size as usize);
    }

    /// Function to implement writing to SD card
    fn write_sector(&mut self, _n: u64, _buf: &[u8]) -> io::Result<usize> {
        unimplemented!("SD card and file system are read only")
    }
}
