pub mod key;
pub mod opts;

use {
    crate::str::{const_string::ConstString, copy_str::CopyStr},
    enum_map::EnumMap,
    key::{KeyAction, KeySequence},
    opts::{Argv, Flag},
    phf::phf_map,
    smallvec::SmallVec,
    std::{
        cell::{RefCell, RefMut},
        cmp::{Ordering, PartialOrd},
        ffi::{c_char, c_int, CStr},
        fmt::{self, Display, Formatter},
        fs::File,
        io::{self, stderr, Write},
        num::TryFromIntError,
        ops::DerefMut,
    },
    strum::VariantArray,
};

/// Someone may be compiling without using cargo, so we cannot do `env!("CARGO_PKG_VERSION")`.
const VERSION: &str = "0.1.0";

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
/// Errors that occur during configuration parsing are reported to stderr, as they could be
/// important and [Self::log_file] may be incomplete.
pub struct Config<'a> {
    commands: SmallVec<[&'a str; 8]>,
    log_level: LogLevel,
    log_file: Option<RefCell<File>>,
    key_bindings: EnumMap<KeyAction, SmallVec<[KeySequence<'a>; 2]>>,

    key_action: Option<KeyAction>,
}
impl<'a> Config<'a> {
    pub fn apply_args<I: IntoIterator<Item = S>, S: Into<CopyStr<'a>>>(
        &mut self,
        args: I,
    ) -> Result<(), ApplyError<'a>> {
        let mut parser = Argv::from(args.into_iter().map(<S as Into<CopyStr<'a>>>::into));
        while let Some(flag) = parser.next() {
            let Some(cli_flag) = (match &flag {
                Flag::Short(short) => CliFlags::SHORT.get(short),
                Flag::Long(long) => CliFlags::LONG.get(long.as_ref()),
            }) else {
                return Err(ApplyError::UnknownFlag(flag));
            };

            cli_flag.apply(self, flag, &mut parser)?;
        }
        Ok(())
    }

    /// # SAFETY
    ///
    /// `argc` must be accurate and `argv` must point to owned memory addresses
    pub unsafe fn apply_argv(
        &mut self,
        argc: c_int,
        argv: *const *const c_char,
    ) -> Result<(), ApplyArgvError> {
        if argc < 0 {
            Err(ApplyArgvError::NegativeArgc)
        } else if argv.is_null() {
            Err(ApplyArgvError::NullArgv)
        } else {
            let argc = <c_int as TryInto<usize>>::try_into(argc)?;
            let argv = (0..argc)
                .map(|i| (i, unsafe { argv.add(i) }))
                .filter_map(|(i, ptr)| {
                    // SAFETY: null is checked above
                    let arg = unsafe { (*ptr).as_ref() };
                    if arg.is_none() {
                        eprintln!("ignoring argument {}: located at null", i);
                    }

                    arg.map(|arg| (i, arg))
                })
                .filter_map(|(i, ptr)| match unsafe { CStr::from_ptr(ptr) }.to_str() {
                    Ok(arg) => Some(arg),
                    Err(err) => {
                        eprintln!("ignoring argument {}: {}", i, err);
                        None
                    }
                });

            Ok(self.apply_args(argv)?)
        }
    }

    fn log_with_level<F: FnOnce(&mut dyn Write) -> io::Result<()>>(
        &self,
        level: LogLevel,
        print: F,
    ) {
        match &self.log_file {
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

#[derive(Debug)]
pub enum ApplyArgvError<'a> {
    Apply(ApplyError<'a>),
    NegativeArgc,
    NullArgv,
    TryFromInt(TryFromIntError),
}
impl<'a> From<ApplyError<'a>> for ApplyArgvError<'a> {
    fn from(err: ApplyError<'a>) -> Self {
        Self::Apply(err)
    }
}
impl From<TryFromIntError> for ApplyArgvError<'_> {
    fn from(err: TryFromIntError) -> Self {
        Self::TryFromInt(err)
    }
}
impl Display for ApplyArgvError<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Apply(err) => write!(f, "{}", err),
            Self::NegativeArgc => write!(f, "negative argc is not allowed"),
            Self::NullArgv => write!(f, "null argv is not allowed"),
            Self::TryFromInt(err) => write!(f, "failed to convert argc to an usize: {}", err),
        }
    }
}

#[derive(Debug)]
pub enum ApplyError<'a> {
    Exit,
    FileOpen(CopyStr<'a>, io::Error),
    MissingValue(Flag<'a>),
    UnknownLogLevel(CopyStr<'a>),
    UnknownFlag(Flag<'a>),
    UnknownKeyAction(CopyStr<'a>),
}
impl Display for ApplyError<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exit => Ok(()),
            Self::FileOpen(path, error) => write!(f, "failed to open file `{}`: {}", path, error),
            Self::MissingValue(flag) => write!(f, "flag `{}` is missing an argument", flag),
            Self::UnknownLogLevel(level) => write!(f, "unknown log level: {}", level),
            Self::UnknownFlag(flag) => write!(f, "unknown flag `{}`", flag),
            Self::UnknownKeyAction(action) => write!(f, "unknown key action: {}", action),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, VariantArray)]
enum CliFlags {
    Help,
    Version,
    LogLevel,
    LogOutput,

    KeyAction,
    KeySequence,
}
impl CliFlags {
    const SHORT: phf::Map<char, CliFlags> = phf_map! {
        'h' => CliFlags::Help,
        'v' => CliFlags::Version,
        'l' => CliFlags::LogLevel,
        'o' => CliFlags::LogOutput,

        'k' => CliFlags::KeyAction,
        'K' => CliFlags::KeySequence,
    };
    const LONG: phf::Map<&str, CliFlags> = phf_map! {
        "help" => CliFlags::Help,
        "version" => CliFlags::Version,
        "log-level" => CliFlags::LogLevel,
        "log-output" => CliFlags::LogOutput,

        "key-action" => CliFlags::KeyAction,
        "key-sequence" => CliFlags::KeySequence,
    };

    const fn short_flag(&self) -> char {
        match self {
            Self::Help => 'h',
            Self::Version => 'v',
            Self::LogLevel => 'l',
            Self::LogOutput => 'o',

            Self::KeyAction => 'k',
            Self::KeySequence => 'K',
        }
    }
    const fn long_flag(&self) -> &'static str {
        match self {
            Self::Help => "help",
            Self::Version => "version",
            Self::LogLevel => "log-level",
            Self::LogOutput => "log-output",

            Self::KeyAction => "key-action",
            Self::KeySequence => "key-sequence",
        }
    }

    const fn short_flags_max_len() -> usize {
        let mut max = 0;
        let mut i = 0;

        while i < Self::VARIANTS.len() {
            let len = Self::VARIANTS[i].short_flag().len_utf8();
            if len > max {
                max = len;
            }

            i += 1;
        }

        max
    }
    const fn long_flags_max_len() -> usize {
        let mut max = 0;
        let mut i = 0;

        while i < Self::VARIANTS.len() {
            let len = Self::VARIANTS[i].long_flag().len();
            if len > max {
                max = len;
            }

            i += 1;
        }

        max
    }

    /// Get the length of padding for lines.
    const fn padding_len() -> usize {
        // `  -`
        3
            + Self::short_flags_max_len()
            // ` `
            + 1
            // `--`
            + 2
            + Self::long_flags_max_len()
            // ` `
            + 1
    }
    const fn padding() -> ConstString<{ Self::padding_len() }> {
        ConstString::new_filled(b' ')
    }

    const fn help(&self) -> &'static [&'static str] {
        match self {
            Self::Help => &["Print this message and exit."],
            Self::Version => &["Print version information and exit."],
            Self::LogLevel => &[
                "Change which log messages are shown.",
                "Levels:",
                "  - none    : Disable all log messages.",
                "  - quiet   : Only show error messages.",
                "  - verbose : Show all log messages.",
            ],
            Self::LogOutput => &[
                "Set which file to print logs.",
                "If unset, defaults to stderr.",
            ],
            Self::KeyAction => &[
                "Set the current key action that all new key bindings belong to.",
                "Actions:",
                "  - quit : End the window manager.",
            ],
            Self::KeySequence => &[
                "A sequence of keys that executes the current key action",
                "Syntax:",
                "  - Most key sequences that can be represented using text simply use text.",
                "    For example, in order to use the sequence `hello`, just type `-Khello`",
                "  - Keys that cannot be printed, escape them in brackets and use their corresponding code.",
                "    Codes:",
                "      - F-{N} : Function key N, where N is a number. (E.g. <F-1> is the f1 key).",
                "      - PG-UP : Page up.",
                "      - PG-DN : Page down.",
                "  - Modifier keys use the the modifier head followed by a dash. (E.g. C-f is control f)",
                "    Heads:",
                "      - C : Control.",
                "      - L : Logo/super.",
                "      - M : Alt.",
                "      - S : Shift.",
            ],
        }
    }
    /// Get the length of [Self::help] with padding and newlines.
    const fn help_len(&self) -> usize {
        let lines = self.help();

        let mut i = 0;
        let mut len = 0;
        while i < lines.len() {
            len += lines[i].len() + 1 + Self::padding_len();
            i += 1;
        }

        len
    }
    /// Get the sum of all the help messages
    const fn help_len_all() -> usize {
        let mut sum = 0;
        let mut i = 0;
        while i < Self::VARIANTS.len() {
            sum += Self::VARIANTS[i].help_len();
            i += 1;
        }

        sum
    }

    /// Write all help texts onto `buffer`
    const fn write_help_all<const N: usize>(buffer: &mut ConstString<N>) {
        let mut i = 0;
        while i < Self::VARIANTS.len() {
            Self::VARIANTS[i].write_help(buffer);
            i += 1;
        }
    }
    /// Write `self`'s help message onto `string`
    const fn write_help<const N: usize>(&self, string: &mut ConstString<N>) {
        const fn pad<const N: usize>(string: &mut ConstString<N>, mut from: usize, to: usize) {
            while from < to {
                string.push(' ');
                from += 1;
            }
        }

        string.push_str("  -");
        let flag = self.short_flag();
        string.push(flag);
        pad(string, flag.len_utf8(), Self::short_flags_max_len());

        string.push_str(" --");
        let flag = self.long_flag();
        string.push_str(flag);
        pad(string, flag.len(), Self::long_flags_max_len());

        string.push(' ');

        let lines = self.help();
        string.push_str(lines[0]);
        string.push('\n');

        let mut i = 1;
        while i < lines.len() {
            string.push_str(Self::padding().as_str());
            string.push_str(lines[i]);
            string.push('\n');
            i += 1;
        }
    }

    fn apply<'a, I>(
        &self,
        config: &mut Config<'a>,
        flag: Flag<'a>,
        argv: &mut Argv<'a, I>,
    ) -> Result<(), ApplyError<'a>>
    where
        I: Iterator<Item = CopyStr<'a>>,
    {
        let value = move || argv.value().ok_or(ApplyError::MissingValue(flag));

        match self {
            Self::Help => {
                const HEAD: &str = "usage: storm [OPTIONS..]\n\n";
                const TAIL: &str = "";
                const TEXT: ConstString<{ HEAD.len() + CliFlags::help_len_all() + TAIL.len() }> = {
                    let mut text = ConstString::new();
                    text.push_str(HEAD);
                    CliFlags::write_help_all(&mut text);
                    text.push_str(TAIL);

                    text
                };

                println!("{}", const { TEXT.as_str() });
                Err(ApplyError::Exit)
            }
            Self::Version => {
                println!("storm {}", VERSION,);
                Err(ApplyError::Exit)
            }
            Self::LogLevel => {
                let value = value()?;

                match value.as_ref() {
                    "none" => {
                        config.log_level = LogLevel::None;
                        Ok(())
                    }
                    "quiet" => {
                        config.log_level = LogLevel::Quiet;
                        Ok(())
                    }
                    "verbose" => {
                        config.log_level = LogLevel::Verbose;
                        Ok(())
                    }
                    _ => Err(ApplyError::UnknownLogLevel(value)),
                }
            }
            Self::LogOutput => {
                let value = value()?;
                config.log_file = Some(RefCell::new(
                    File::open(value.as_ref()).map_err(|err| ApplyError::FileOpen(value, err))?,
                ));
                Ok(())
            }
            Self::KeyAction => {
                let value = value()?;

                match value.as_ref() {
                    "quit" => {
                        config.key_action = Some(KeyAction::Quit);
                        Ok(())
                    },
                    _ => Err(ApplyError::UnknownKeyAction(value)),
                }
            }
            Self::KeySequence => {
                let _value = value()?;

                todo!("parse `value` & change Parser::parse to use `CopyStr`s")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_exist() {
        CliFlags::VARIANTS.iter().copied().for_each(|flag| {
            assert!(CliFlags::SHORT
                .values()
                .chain(CliFlags::LONG.values())
                .copied()
                .any(|cli_flag| flag == cli_flag));
        })
    }

    #[test]
    fn logging() {
        fn log_map<F: FnMut(LogLevel) -> bool>(log_level: LogLevel, mut expected: F) {
            let config = Config {
                log_level,
                ..Default::default()
            };

            [LogLevel::None, LogLevel::Quiet, LogLevel::Verbose]
                .into_iter()
                .map(|level| (level, expected(level)))
                .for_each(|(level, expected)| {
                    let mut logged = false;
                    config.log_with_level(level, |_| {
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

    #[test]
    fn version_sync() {
        if let Some(cargo_version) = option_env!("CARGO_PKG_VERSION") {
            assert_eq!(cargo_version, VERSION);
        }
    }

    #[test]
    fn cli_flags_serde() {
        CliFlags::VARIANTS
            .iter()
            .map(|flag| (flag.short_flag(), flag.long_flag(), flag))
            .for_each(|(short, long, into)| {
                assert_eq!(CliFlags::SHORT.get(&short), Some(into));
                assert_eq!(CliFlags::LONG.get(&long), Some(into));
            })
    }
}
