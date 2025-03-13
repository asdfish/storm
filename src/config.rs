use {
    crate::opts::{Flag, Parser},
    std::{
        ffi::{CStr, c_char, c_int},
        process::exit,
    },
};

#[derive(Default)]
pub struct Config<'a> {
    commands: Vec<&'a str>,
    pub verbosity: Verbosity,
}
impl<'a> Config<'a> {
    /// SAFETY: `argc` must be accurate and `argv` must point to owned memory addresses
    pub unsafe fn apply(&mut self, argc: c_int, argv: *const *const c_char) {
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

            let mut parser = Parser::new(argv);
            while let Some(flag) = parser.next() {
                macro_rules! value_or_continue {
                    () => {
                        match parser.value(flag) {
                            Ok(value) => value,
                            Err(err) => {
                                self.verbosity.error(|| eprintln!("{}", err));
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
  -h --help    display this message and exit
  -V --version show version information and exit
  -v --verbose set how verbose logging should be
               none    - disable logging
               quiet   - only log errors
               verbose - log progress
  -c --command command to execute in wayland display"
                        );

                        exit(0);
                    }
                    Flag::Short('V') | Flag::Long("version") => {
                        println!("storm {}", env!("CARGO_PKG_VERSION"));
                        exit(0);
                    }
                    Flag::Short('v') | Flag::Long("verbose") => match value_or_continue!() {
                        "none" => self.verbosity = Verbosity::None,
                        "quiet" => self.verbosity = Verbosity::Quiet,
                        "verbose" => self.verbosity = Verbosity::Verbose,
                        level => self
                            .verbosity
                            .error(|| eprintln!("unknown verbosity level: `{}`", level)),
                    },
                    Flag::Short('c') | Flag::Long("command") => {
                        self.commands.push(value_or_continue!());
                    }
                    flag @ Flag::Short(_) | flag @ Flag::Long(_) => {
                        self.verbosity
                            .error(|| eprintln!("unknown flag `{}`", flag));
                    }
                    _ => {}
                }
            }
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq)]
pub enum Verbosity {
    None,
    #[default]
    Quiet,
    Verbose,
}
impl Verbosity {
    /// Only activates on [Self::Quiet] and [Self::Verbose]
    pub fn error<F>(&self, operation: F)
    where
        F: FnOnce(),
    {
        if *self != Verbosity::None {
            operation();
        }
    }
    /// Only activates on [Self::Verbose]
    pub fn log<F>(&self, operation: F)
    where
        F: FnOnce(),
    {
        if *self == Verbosity::Verbose {
            operation();
        }
    }
}
