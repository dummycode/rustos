mod parsers;

use serial;
use structopt;
use structopt_derive::StructOpt;
use xmodem::Xmodem;
use xmodem::Progress;

use std::path::PathBuf;
use std::time::Duration;

use shim::io;

use structopt::StructOpt;
use serial::core::{CharSize, BaudRate, StopBits, FlowControl, SerialDevice, SerialPortSettings};

use parsers::{parse_width, parse_stop_bits, parse_flow_control, parse_baud_rate};

#[derive(StructOpt, Debug)]
#[structopt(about = "Write to TTY using the XMODEM protocol by default.")]
struct Opt {
    #[structopt(short = "i", help = "Input file (defaults to stdin if not set)", parse(from_os_str))]
    input: Option<PathBuf>,

    #[structopt(short = "b", long = "baud", parse(try_from_str = "parse_baud_rate"), help = "Set baud rate", default_value = "115200")]
    baud_rate: BaudRate,

    #[structopt(short = "t", long = "timeout", parse(try_from_str), help = "Set timeout in seconds", default_value = "10")]
    timeout: u64,

    #[structopt(short = "w", long = "width", parse(try_from_str = "parse_width"), help = "Set data character width in bits", default_value = "8")]
    char_width: CharSize,

    #[structopt(help = "Path to TTY device", parse(from_os_str))]
    tty_path: PathBuf,

    #[structopt(short = "f", long = "flow-control", parse(try_from_str = "parse_flow_control"), help = "Enable flow control ('hardware' or 'software')", default_value = "none")]
    flow_control: FlowControl,

    #[structopt(short = "s", long = "stop-bits", parse(try_from_str = "parse_stop_bits"), help = "Set number of stop bits", default_value = "1")]
    stop_bits: StopBits,

    #[structopt(short = "r", long = "raw", help = "Disable XMODEM")]
    raw: bool,
}

fn progress_fn(progress: Progress) {
    // Do nothing
}

fn send_it<R, W>(mut from: R, mut into: W, raw: bool) -> io::Result<usize>
where W: io::Read + io::Write, R: io::Read {
    use std::io::{copy};
    let size: usize = if raw {
        copy(&mut from, &mut into)? as usize
    } else {
        Xmodem::transmit_with_progress(from, into, progress_fn)?
    };

    return Ok(size);
}

fn main() {
    use std::fs::File;
    use std::error::Error;

    let opt = Opt::from_args();
    let mut port = serial::open(&opt.tty_path).expect("Path points to invalid TTY");

    let mut settings = port.read_settings().expect("Cannot read settings");

    settings.set_baud_rate(opt.baud_rate).expect("Baud rate is invalid");
    settings.set_char_size(opt.char_width);
    settings.set_flow_control(opt.flow_control);
    settings.set_stop_bits(opt.stop_bits);

    port.write_settings(&settings).expect("Settings are not valid");

    port.set_timeout(Duration::new(opt.timeout, 0)).expect("Timeout not valid");

    let result;
    if opt.input == None {
        let data = io::stdin();
        result = send_it(data, port, opt.raw);
    } else {
        let input = opt.input.unwrap();
        let data = match File::open(input) {
            Err(why) => panic!(
                "couldn't open file – {}",
                why.description()
            ),
            Ok(file) => file,
        };
        result = send_it(data, port, opt.raw);
    }

    match result {
        Err(why) => println!("Error sending it: {}", why.description()),
        Ok(size) => println!("{} bytes transmitted", size),
    };
}

