pub mod key;
pub mod opts;

use {
    crate::str::{
        copy_str::CopyStr,
        const_string::ConstString,
    },
    enum_map::EnumMap,
    key::{Key, KeyAction},
    opts::{Argv, Flag},
    phf::phf_map,
    smallvec::SmallVec,
    std::{
        cell::{RefCell, RefMut},
        cmp::{Ordering, PartialOrd},
        ffi::{c_char, c_int, CStr},
        fs::File,
        io::{self, stderr, Write},
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
    key_bindings: EnumMap<KeyAction, SmallVec<[Key<'a>; 4]>>,
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
    ///
    /// Errors in argument parsing are always printed to stderr.
    // TODO: change this to `impl TryFrom<(c_int, *const *const c_char)> for Argv {}`
    pub unsafe fn apply_argv(&mut self, argc: c_int, argv: *const *const c_char) {
        if argc > 0 && !argv.is_null() {
            let argv = (0..argc)
                .skip(1)
                // SAFETY: null check is above
                .map(|i| {
                    argv.wrapping_offset(
                        i.try_into()
                            .expect("internal error: argc should be filtered to be positive above"),
                    )
                })
                .filter_map(|arg| {
                    // SAFETY: will never be null
                    let arg = unsafe { *arg };
                    if arg.is_null() {
                        None
                    } else {
                        Some(arg)
                    }
                })
                // SAFETY: null checking is performed above
                .map(|arg| unsafe { CStr::from_ptr(arg) })
                .filter_map(|arg| match arg.to_str() {
                    Ok(arg) => Some(arg),
                    Err(err) => {
                        eprintln!("ignoring argument `{:?}`: {}", arg, err);
                        None
                    }
                });
            self.apply_args(argv).unwrap();
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
pub enum ApplyError<'a> {
    Exit,
    MissingValue(Flag<'a>),
    UnknownFlag(Flag<'a>),
}
impl ApplyError<'_> {
    pub const fn exit_code(&self) -> c_int {
        matches!(self, Self::Exit) as c_int
    }
}

#[derive(Clone, Copy, Debug, PartialEq, VariantArray)]
enum CliFlags {
    Help,
    Version,
    LogLevel,
    LogOutput,
}
impl CliFlags {
    const SHORT: phf::Map<char, CliFlags> = phf_map! {
        'h' => CliFlags::Help,
        'v' => CliFlags::Version,
        'l' => CliFlags::LogLevel,
        'o' => CliFlags::LogOutput,
    };
    const LONG: phf::Map<&str, CliFlags> = phf_map! {
        "help" => CliFlags::Help,
        "version" => CliFlags::Version,
        "log-level" => CliFlags::LogLevel,
        "log-output" => CliFlags::LogOutput,
    };

    const fn short_flag(&self) -> char {
        match self {
            Self::Help => 'h',
            Self::Version => 'v',
            Self::LogLevel => 'l',
            Self::LogOutput => 'o',
        }
    }
    const fn long_flag(&self) -> &'static str {
        match self {
            Self::Help => "help",
            Self::Version => "version",
            Self::LogLevel => "log-level",
            Self::LogOutput => "log-output",
        }
    }

    /// # Safety
    ///
    /// This is safe if `tests::cli_flags_length_invariants` passes
    const unsafe fn short_flags_max_len() -> usize {
        1
    }
    /// # Safety
    ///
    /// This is safe if `tests::cli_flags_length_invariants` passes
    const unsafe fn long_flags_max_len() -> usize {
        10
    }

    /// Get the length of padding for lines.
    const fn padding_len() -> usize {
        // `  -`
        3
            + unsafe { Self::short_flags_max_len() }
            // ` `
            + 1
            // `--`
            + 2
            + unsafe { Self::long_flags_max_len() }
            // ` `
            + 1
    }
    const fn padding() -> ConstString<{ Self::padding_len() }> {
        ConstString::new_filled(b' ')
    }

    const fn help(&self) -> &'static [&'static str] {
        match self {
            Self::Help => &["print this message and exit"],
            Self::Version => &["print version information and exit"],
            Self::LogLevel => &[
                "change which log messages are shown",
                "Levels:",
                "  - none    : disable all log messages",
                "  - quiet   : only show error messages",
                "  - verbose : show all log messages",
            ],
            Self::LogOutput => &["set which file to print logs", "defaults to stderr"],
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
    /// Get the sum of all the lengths
    const fn help_len_all() -> usize {
        let mut sum = 0;
        let mut i = 0;
        while i < Self::VARIANTS.len() {
            sum += Self::VARIANTS[i].help_len();
            i += 1;
        }

        sum
    }
    const fn write_help_all<const N: usize>(buffer: &mut ConstString<N>) {
        let mut i = 0;
        while i < Self::VARIANTS.len() {
            Self::VARIANTS[i].write_help(buffer);
            i += 1;
        }
    }

    /// Write help onto `string`
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
        pad(string, flag.len_utf8(), unsafe { Self::short_flags_max_len() });

        string.push_str(" --");
        let flag = self.long_flag();
        string.push_str(flag);
        pad(string, flag.len(), unsafe { Self::long_flags_max_len() });

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
                const HEAD: &str = "usage: storm [OPTIONS..]";
                const TAIL: &str = "";
                const TEXT: ConstString::<{
                    HEAD.len()
                        + CliFlags::help_len_all()
                        + TAIL.len()
                }> = {
                    let mut text = ConstString::new();
                    text.push_str(HEAD);
                    CliFlags::write_help_all(&mut text);
                    text.push_str(TAIL);

                    text
                };

                println!("{}", const { TEXT.as_str() });
                Err(ApplyError::Exit)
            },
            Self::Version => {
                println!(
                    "storm {}",
                    VERSION,
                );
                Err(ApplyError::Exit)
            }
            _ => todo!(),
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
    // these actually don't need to be tested since if invariants are violated, it would just create a compile time panic
    fn cli_flags_length_invariants() {
        fn max_key_len<K, V, F>(map: phf::Map<K, V>, len_utf8: F) -> usize
        where F: for<'a> FnMut(&'a K) -> usize {
            assert!(!map.is_empty());

            map
                .keys()
                .map(len_utf8)
                .inspect(|len| assert!(*len != 0))
                .max()
                .unwrap_or(0)
        }

        assert_eq!(
            max_key_len(CliFlags::SHORT, |ch| ch.len_utf8()),
            // SAFETY: only used for comparison
            unsafe { CliFlags::short_flags_max_len() }
        );
        assert_eq!(
            max_key_len(CliFlags::LONG, |ch| ch.len()),
            // SAFETY: only used for comparison
            unsafe { CliFlags::long_flags_max_len() }
        );
    }
}
