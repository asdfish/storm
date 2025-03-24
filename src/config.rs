pub mod file_parser;
pub mod key;
pub mod opts;

use {
    crate::const_string::ConstString,
    either::Either,
    enum_map::EnumMap,
    key::{KeyAction, KeySequence, Parser, ParserError},
    opts::{Argv, Flag},
    phf::phf_map,
    smallvec::SmallVec,
    std::{
        cmp::{Ordering, PartialOrd},
        ffi::{CStr, c_char, c_int},
        fmt::{self, Display, Formatter},
        fs::File,
        io::{self, Write, stderr},
        num::TryFromIntError,
        str::Utf8Error,
    },
    strum::VariantArray,
};

/// Someone may be compiling without using cargo, so we cannot do `env!("CARGO_PKG_VERSION")`.
const VERSION: &str = "0.1.0";

#[derive(Clone, Copy, Debug, Default, PartialEq)]
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

#[derive(Debug, Default)]
/// Errors that occur during configuration parsing are reported to stderr, as they could be
/// important and [Self::log_file] may be incomplete.
pub struct Config<'a> {
    commands: SmallVec<[&'a str; 8]>,
    log_level: LogLevel,
    log_file: Option<File>,
    key_bindings: EnumMap<KeyAction, SmallVec<[KeySequence<'a>; 2]>>,

    key_action: Option<KeyAction>,
}
impl<'a> Config<'a> {
    /// Remove state
    pub fn clean_state(&mut self) {
        self.key_action = None;
    }

    pub fn apply_args<I: IntoIterator<Item = Result<&'a S, E>>, S: AsRef<str> + ?Sized + 'a, E>(
        &mut self,
        args: I,
    ) -> Result<(), ApplyError<'a, E>>
    where
        E: Display,
    {
        let mut parser = Argv::from(args.into_iter().map(|arg| arg.map(|arg| arg.as_ref())));
        while let Some(flag) = parser.next() {
            let flag = flag.map_err(ApplyError::ArgSource)?;
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
    ) -> Result<(), Either<ApplyArgvError, ApplyError<ApplyArgvError>>> {
        if argc < 0 {
            Err(Either::Left(ApplyArgvError::NegativeArgc))
        } else if argv.is_null() {
            Err(Either::Left(ApplyArgvError::NullArgv))
        } else {
            let argc = <c_int as TryInto<usize>>::try_into(argc)
                .map_err(ApplyArgvError::TryFromInt)
                .map_err(Either::Left)?;
            let argv = (0..argc)
                .map(|i| (i, unsafe { argv.add(i) }))
                .map(|(i, ptr)| {
                    // SAFETY: null is checked above
                    match unsafe { (*ptr).as_ref() } {
                        Some(ptr) => Ok((i, ptr)),
                        None => Err(ApplyArgvError::NullArg(i)),
                    }
                })
                .map(|arg| {
                    let (i, ptr) = arg?;
                    unsafe { CStr::from_ptr(ptr) }
                        .to_str()
                        .map_err(|err| ApplyArgvError::Utf8(i, err))
                });

            self.apply_args(argv).map_err(Either::Right)
        }
    }

    fn log_with_level<F: FnOnce(&mut dyn Write) -> io::Result<()>>(
        &mut self,
        level: LogLevel,
        print: F,
    ) {
        match &mut self.log_file {
            Some(file) => self.log_level.log(
                level,
                file,
                print,
            ),
            None => self.log_level.log(level, &mut stderr(), print),
        }
    }

    pub fn log<F: FnOnce(&mut dyn Write) -> io::Result<()>>(&mut self, print: F) {
        self.log_with_level(LogLevel::Verbose, print)
    }
    pub fn error<F: FnOnce(&mut dyn Write) -> io::Result<()>>(&mut self, print: F) {
        self.log_with_level(LogLevel::Quiet, print)
    }
}

#[derive(Debug)]
pub enum ApplyArgvError {
    NegativeArgc,
    NullArg(usize),
    NullArgv,
    TryFromInt(TryFromIntError),
    Utf8(usize, Utf8Error),
}
impl From<TryFromIntError> for ApplyArgvError {
    fn from(err: TryFromIntError) -> Self {
        Self::TryFromInt(err)
    }
}
impl Display for ApplyArgvError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::NegativeArgc => write!(f, "negative argc is not allowed"),
            Self::NullArg(i) => write!(f, "argument {} is null", i),
            Self::NullArgv => write!(f, "null argv is not allowed"),
            Self::TryFromInt(err) => write!(f, "failed to convert argc to an usize: {}", err),
            Self::Utf8(i, err) => write!(f, "argument {} contains invalid utf8: {}", i, err),
        }
    }
}

#[derive(Debug)]
pub enum ApplyError<'a, E> 
where
    E: Display,
{
    ArgSource(E),
    Exit,
    FileOpen(&'a str, io::Error),
    KeyParser(key::ParserError<'a>),
    MissingValue(Flag<'a>),
    UnknownLogLevel(&'a str),
    UnknownFlag(Flag<'a>),
    UnknownKeyAction(&'a str),
    UnsetKeyAction,
}
impl<E> Display for ApplyError<'_, E>
where
    E: Display,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::ArgSource(err) => write!(f, "failed to source arguments: {}", err),
            Self::Exit => Ok(()),
            Self::FileOpen(path, error) => write!(f, "failed to open file `{}`: {}", path, error),
            Self::KeyParser(err) => write!(f, "failed to parse keys: {}", err),
            Self::MissingValue(flag) => write!(f, "flag `{}` is missing an argument", flag),
            Self::UnknownLogLevel(level) => write!(f, "unknown log level: {}", level),
            Self::UnknownFlag(flag) => write!(f, "unknown flag `{}`", flag),
            Self::UnknownKeyAction(action) => write!(f, "unknown key action: {}", action),
            Self::UnsetKeyAction => write!(f, "`key-action` is not set"),
        }
    }
}
impl<'a, E> From<ParserError<'a>> for ApplyError<'a, E>
where
    E: Display,
{
    fn from(err: ParserError<'a>) -> Self {
        Self::KeyParser(err)
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

    fn apply<'a, I, E>(
        &self,
        config: &mut Config<'a>,
        flag: Flag<'a>,
        argv: &mut Argv<'a, I, E>,
    ) -> Result<(), ApplyError<'a, E>>
    where
        I: Iterator<Item = Result<&'a str, E>>,
        E: Display,
    {
        let mut value = move || match argv.value() {
            Some(Ok(val)) => Ok(val),
            Some(Err(err)) => Err(ApplyError::ArgSource(err)),
            None => Err(ApplyError::MissingValue(flag)),
        };

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

                match value {
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
                config.log_file = Some(File::open(value).map_err(|err| ApplyError::FileOpen(value, err))?,);
                Ok(())
            }
            Self::KeyAction => {
                let value = value()?;

                match value {
                    "quit" => {
                        config.key_action = Some(KeyAction::Quit);
                        Ok(())
                    }
                    _ => Err(ApplyError::UnknownKeyAction(value)),
                }
            }
            Self::KeySequence => {
                if let Some(action) = config.key_action {
                    let value = value()?;

                    if let Some(key_sequence) = KeySequence::parse(value).transpose()? {
                        config.key_bindings[action].push(key_sequence.0);
                    }

                    Ok(())
                } else {
                    Err(ApplyError::UnsetKeyAction)
                }
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
            assert!(
                CliFlags::SHORT
                    .values()
                    .chain(CliFlags::LONG.values())
                    .copied()
                    .any(|cli_flag| flag == cli_flag)
            );
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
