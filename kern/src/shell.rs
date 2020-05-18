use shim::io;
use shim::path::{Path, PathBuf};

use core::str;
use core::str::FromStr;
use core::time::Duration;

use alloc::vec::Vec;
use alloc::string::String;

use pi::atags::Atags;

use shim::io::Read;
use fat32::traits::FileSystem as FileSystemTrait;
use fat32::traits::{Dir as DirTrait, Entry as EntryTrait, File as FileTrait};
use fat32::vfat::{Dir, Entry, File, VFat, VFatHandle};

use crate::console::{kprint, kprintln, CONSOLE};
use crate::ALLOCATOR;
use crate::FILESYSTEM;

use kernel_api::*;

/// Error type for `Command` parse failures.
#[derive(Debug, PartialEq)]
enum Error {
    Empty,
    TooManyArgs,
}

/// A structure representing a single shell command.
struct Command<'a> {
    args: Vec<&'a str>,
}

impl<'a> Command<'a> {
    /// Parse a command from a string `s` using `buf` as storage for the
    /// arguments.
    ///
    /// # Errors
    ///
    /// If `s` contains no arguments, returns `Error::Empty`. If there are more
    /// arguments than `buf` can hold, returns `Error::TooManyArgs`.
    fn parse(s: &'a str, buf: &'a mut [&'a str]) -> Result<Command<'a>, Error> {
        let mut args: Vec<&str> = Vec::new();
        for arg in s.split(' ').filter(|a| !a.is_empty()) {
            args.push(arg);
        }

        if args.is_empty() {
            return Err(Error::Empty);
        }

        Ok(Command { args })
    }

    /// Returns this command's path. This is equivalent to the first argument.
    fn path(&self) -> &str {
        return self.args.as_slice()[0];
    }
}

pub struct Shell {
    current_path: PathBuf,
    prefix: String,
}

impl Shell {
    /// Get new shell
    pub fn new(prefix: String) -> Shell {
        return Shell {
            current_path: PathBuf::from("/"),
            prefix,
        };
    }

    /// String to path, returns None if the path isn't valid
    fn path_string_to_path(&self, path: &str) -> Option<PathBuf> {
        let target = Path::new(path);
        let joined = self.current_path.as_path().join(target);
        if FILESYSTEM.open_dir(&joined).is_err() && FILESYSTEM.open_file(&joined).is_err() {
            return None;
        }

        return Some(joined);
    }

    /// Handler for `cat`
    fn cat_handler(&self, args: &Vec<&str>) {
        if args.len() < 2 {
            kprintln!("cat: not enough arguments");
            return;
        }
        // Cat every arg
        for arg in args[1..].to_vec() {
            match self.path_string_to_path(arg) {
                Some(path) => {
                    if path.as_path().extension().is_none() {
                        kprintln!("cat: {}: Is a directory", path.as_os_str().to_str().unwrap());
                        return;
                    }
                    let mut res = FILESYSTEM.open_file(path).unwrap();
                    let mut buf: [u8; 512] = [0; 512];
                    let size = res.read(&mut buf).expect("Expected file size");
                    let contents = String::from_utf8(buf[..size].to_vec()).expect("Excepted valid contents");
                    kprintln!("{}", contents);
                },
                None => {
                    kprintln!("cat: {}: No such file or directory", arg);
                },
            }
        }
    }

    /// Handler for `pwd`
    fn pwd_handler(&self, args: &Vec<&str>) {
        if args.len() > 1 {
            kprintln!("pwd: too many arguments");
            return;
        }

        kprintln!("{}", self.current_path.as_os_str().to_str().expect("Expected valid string"));
    }

    /// Handler for `ls`
    fn ls_handler(&self, args: &Vec<&str>) {
        for entry in FILESYSTEM.open(&self.current_path).unwrap().into_dir().unwrap().entries().unwrap() {
            match entry {
                Entry::EntryFile(file) => kprintln!("{}" , file.name),
                Entry::EntryDir(dir) => kprintln!("{}" , dir.name),
            }
        }
    }

    /// Handle an `echo` command
    fn echo_handler(&self, args: &Vec<&str>) {
        for arg in args[1..].to_vec() {
            kprint!("{} ", arg);
        }
        kprintln!("");
    }

    /// Handler for `cd`
    fn cd_handler(&mut self, args: &Vec<&str>) {
        if args.len() == 1 {
            // cd to root directory
            self.current_path = PathBuf::from("/");
            return;
        }
        if args.len() > 2 {
            kprintln!("cd: too many arguments");
            return;
        }

        let target: &Path = Path::new(args[1]);
        let copy = self.current_path.as_path();
        if FILESYSTEM.open_dir(copy.join(target)).is_ok() {
            self.current_path.push(target);
        } else {
            kprintln!("cd: no such file or directory: {}", args[1]);
            kprintln!("{:?}", copy.join(target));
        }
    }

    /// Handler for `sleep`
    fn sleep_handler(&mut self, args: &Vec<&str>) {
        if args.len() != 2 {
            kprintln!("usage: sleep ms");
            return;
        }

        match u32::from_str(args[1]) {
            Ok(ms) => {
                kprintln!("Sleeping for {}ms", ms);

                let duration = Duration::from_millis(ms as u64);
                syscall::sleep(duration);
            },
            Err(_) => {
                kprintln!("usage: sleep ms");
            },
        }
    }


    /// Starts a shell using `prefix` as the prefix for each line. This function
    /// never returns.
    pub fn shell(&mut self) {
        let mut console = CONSOLE.lock();

        kprint!("{}", self.prefix);
        let mut total = 0;
        let mut line = Vec::new();

        loop {
            let byte = console.read_byte();

            if byte == b'\n' || byte == b'\r' {
                kprintln!("");

                let string = str::from_utf8(line.as_slice());
                match string {
                    Err(e) => kprintln!("Error: could not convert command to utf8"),
                    Ok(s) => {
                        let mut args = unsafe { [str::from_utf8_unchecked(&[0; 512]); 64] };
                        let result = Command::parse(s, &mut args);
                        match result {
                            Err(e) => {
                                if e == Error::TooManyArgs {
                                    kprintln!("Error: too many arguments");
                                }
                            },
                            Ok(command) => {
                                match &command.path() {
                                    &"echo" => self.echo_handler(&command.args),
                                    &"ls" => self.ls_handler(&command.args),
                                    &"cd" => self.cd_handler(&command.args),
                                    &"pwd" => self.pwd_handler(&command.args),
                                    &"about" => {
                                        kprintln!("Henry Harris' Operating System (HHOS)");
                                        kprintln!("CS 3210 - Georgia Institute of Technology");
                                    },
                                    &"yeet" => {
                                        panic!("Yeeted on");
                                    },
                                    &"cat" => self.cat_handler(&command.args),
                                    &"sleep" => self.sleep_handler(&command.args),
                                    &"exit" => { 
                                        kprintln!("Exiting shell...");
                                        return; 
                                    }
                                    _ => kprintln!("HHsh: command not found: {}", command.path()),
                                };
                            }
                        }
                    }
                }

                kprint!("{}", self.prefix);
                total = 0;
                line = Vec::new();
                continue;

            }

            if byte == 127 {
                if total > 0 {
                    kprint!("\x08 \x08");
                    total -= 1;
                    line.pop();
                }
                continue;
            }

            if byte < 32 || byte > 126 {
                kprint!("\x07");
                continue;
            }

            if total < 512 {
                console.write_byte(byte);
                total += 1;
                line.push(byte);
            }
        }
        loop {}
    }
}

