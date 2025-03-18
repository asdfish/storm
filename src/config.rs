mod opts;

use {
    opts::{Flag, Parser},
    std::{
        cell::{RefCell, RefMut},
        cmp::{Ordering, PartialOrd},
        ffi::{CStr, c_char, c_int},
        fs::File,
        io::{self, Write, stderr},
        ops::DerefMut,
        process::{Command, exit},
    },
};

#[cfg_attr(test, derive(enum_map::Enum))]
#[derive(Clone, Copy, Default, PartialEq)]
#[repr(u8)]
/// Determines how verbose log messages should be.
enum LogLevel {
    None,
    #[default]
    Quiet,
    Verbose,
}
impl PartialOrd for LogLevel {
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        match (self, rhs) {
            (&Self::None, _) | (_, &Self::None) => None,
            _ => (*self as u8).partial_cmp(&(*rhs as u8)),
        }
    }
}
impl LogLevel {
    /// Compares the current log level with `level` and executes the function if the level is
    /// higher than [self].
    fn log<F: FnOnce(&mut dyn Write) -> io::Result<()>>(
        &self,
        level: Self,
        file: &mut dyn Write,
        print: F,
    ) {
        if *self >= level {
            if let Err(err) = print(file) {
                eprintln!("error while logging: {}", err);
            }
        }
    }
}

#[derive(Default)]
pub struct Config<'a> {
    commands: Vec<&'a str>,
    log_level: LogLevel,
    file: Option<RefCell<File>>,
}
impl<'a> Config<'a> {
    /// Errors in argument parsing are always printed to stderr.
    pub fn apply<I: Iterator<Item = &'a S>, S: AsRef<str> + ?Sized + 'a>(&mut self, args: I) {
        let mut parser = Parser::new(args);
        while let Some(flag) = parser.next() {
            macro_rules! value_or_continue {
                () => {
                    match parser.value(flag) {
                        Ok(value) => value,
                        Err(err) => {
                            eprintln!("{}", err);
                            continue;
                        }
                    }
                };
            }

            match flag {
                Flag::Short('h') | Flag::Long("help") => {
                    println!(
                        "usage: storm [OPTIONS]...

Options:
-h --help    Display this message and exit.
-V --version Show version information and exit.
-v --verbose Set how verbose logging should be:
none    - disable logging
quiet   - only log errors
verbose - log progress
-c --command Command to execute after initialization.
-l --log     File to print logs in.
Defaults to stderr if not set or printing fails."
                    );

                    exit(0);
                }
                Flag::Short('V') | Flag::Long("version") => {
                    println!("storm {}", env!("CARGO_PKG_VERSION"));
                    exit(0);
                }
                Flag::Short('v') | Flag::Long("verbose") => match value_or_continue!() {
                    "none" => self.log_level = LogLevel::None,
                    "quiet" => self.log_level = LogLevel::Quiet,
                    "verbose" => self.log_level = LogLevel::Verbose,
                    level => eprintln!("unknown verbosity level: `{}`", level),
                },
                Flag::Short('c') | Flag::Long("command") => {
                    self.commands.push(value_or_continue!());
                }
                Flag::Short('l') | Flag::Long("log") => {
                    self.file = File::create(value_or_continue!()).map(RefCell::new).ok();
                }
                flag @ Flag::Short(_) | flag @ Flag::Long(_) => {
                    eprintln!("unknown flag `{}`", flag);
                }
                _ => {}
            }
        }
    }

    /// # SAFETY
    ///
    /// `argc` must be accurate and `argv` must point to owned memory addresses
    ///
    /// Errors in argument parsing are always printed to stderr.
    pub unsafe fn apply_argv(&mut self, argc: c_int, argv: *const *const c_char) {
        if argc > 0 {
            let argv = (0..argc)
                .skip(1)
                // SAFETY: argc is not incremented by zero, which makes it never null
                .map(|i| unsafe {
                    argv.add(
                        i.try_into()
                            .expect("argc should be filtered to be positive above"),
                    )
                })
                // SAFETY: the pointer will never be null since the address would always be greater
                // than argv + 1 due to the skip above
                .map(|arg| unsafe { CStr::from_ptr(*arg) })
                .filter_map(|arg| match arg.to_str() {
                    Ok(arg) => Some(arg),
                    Err(err) => {
                        eprintln!("ignoring argument `{:?}`: {}", arg, err);
                        None
                    }
                });
            self.apply(argv);
        }
    }

    pub fn execute_commands(&self) {
        self.commands
            .iter()
            .for_each(|command| match Command::new(command).spawn() {
                Ok(_) => {}
                Err(err) => {
                    self.error(|f| writeln!(f, "error spawning command `{}`: {}", command, err))
                }
            })
    }

    fn log_with_level<F: FnOnce(&mut dyn Write) -> io::Result<()>>(
        &self,
        level: LogLevel,
        print: F,
    ) {
        match &self.file {
            Some(file) => self.log_level.log(
                level,
                <RefMut<'_, File> as DerefMut>::deref_mut(&mut file.borrow_mut()),
                print,
            ),
            None => self.log_level.log(level, &mut stderr(), print),
        }
    }

    pub fn log<F: FnOnce(&mut dyn Write) -> io::Result<()>>(&self, print: F) {
        self.log_with_level(LogLevel::Verbose, print)
    }
    pub fn error<F: FnOnce(&mut dyn Write) -> io::Result<()>>(&self, print: F) {
        self.log_with_level(LogLevel::Quiet, print)
    }
}

#[cfg(test)]
mod tests {
    use {super::*, enum_map::EnumMap};

    #[test]
    fn logging() {
        fn log_map<F: FnMut(LogLevel) -> bool>(log_level: LogLevel, expected: F) {
            let config = Config {
                log_level,
                ..Default::default()
            };

            EnumMap::from_fn(expected)
                .into_iter()
                .for_each(|(level, expected)| {
                    let mut logged = false;
                    config.log(level, |_| {
                        logged = true;
                        Ok(())
                    });

                    assert_eq!(logged, expected);
                });
        }

        log_map(LogLevel::None, |_| false);
        log_map(LogLevel::Quiet, |level| matches!(level, LogLevel::Quiet));
        log_map(LogLevel::Verbose, |level| !matches!(level, LogLevel::None));
    }
}
