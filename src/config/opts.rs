use std::fmt::{self, Display, Formatter};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Flag<'arg> {
    Separator,
    Value,
    Short(char),
    Long(&'arg str),
}
impl Display for Flag<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Short(flag) => write!(f, "-{}", flag),
            Self::Long(flag) => write!(f, "--{}", flag),
            _ => Ok(()),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
/// The private representation of flags with extra metadata.
struct FlagInner<'arg> {
    kind: Flag<'arg>,
    value: &'arg str,
}

impl<'arg> FlagInner<'arg> {
    fn new<S: AsRef<str> + ?Sized + 'arg>(arg: &'arg S) -> (Self, &'arg str) {
        let arg = arg.as_ref();

        if arg == "--" {
            (
                FlagInner {
                    kind: Flag::Separator,
                    value: "",
                },
                "",
            )
        } else if let Some(next) = arg.strip_prefix("--") {
            let (identifier, value) = next
                .char_indices()
                .find(|(_, ch)| *ch == '=')
                .and_then(|(i, _)| Some((&next[..i], &next[i.checked_add(1)?..])))
                .unwrap_or((next, ""));

            (
                FlagInner {
                    kind: Flag::Long(identifier),
                    value,
                },
                next,
            )
        } else if arg.starts_with('-') && arg.len() >= 2 {
            let mut arg = arg.chars();
            arg.next();

            (
                FlagInner {
                    kind: Flag::Short(arg.next().expect(
                        "internal error: the check above should ensure that there is always more at least 2 characters",
                    )),
                    value: arg.as_str(),
                },
                arg.as_str(),
            )
        } else {
            (
                FlagInner {
                    kind: Flag::Value,
                    value: arg,
                },
                arg,
            )
        }
    }

    fn value(&self) -> Option<&'arg str> {
        let value: Option<&'arg str> = match self.kind {
            Flag::Separator => None,
            Flag::Value | Flag::Long(_) => Some(self.value),
            Flag::Short(_) => self
                .value
                .get(if self.value.starts_with('=') { 1 } else { 0 }..),
        };

        value.filter(|value| !value.is_empty())
    }
}
/// A single string in argv.
///
/// The shell command 'ls -l -s -h' contains the flags 'ls', '-l', '-s' and '-h'
#[derive(Clone, Copy, Debug)]
struct Argument<'arg> {
    last: Option<FlagInner<'arg>>,
    text: &'arg str,
}
impl<'arg> Argument<'arg> {
    pub fn new<S: AsRef<str> + ?Sized + 'arg>(text: &'arg S) -> Self {
        Self {
            last: None,
            text: text.as_ref(),
        }
    }
}
impl<'arg> Iterator for Argument<'arg> {
    type Item = FlagInner<'arg>;

    fn next(&mut self) -> Option<FlagInner<'arg>> {
        if self.text.is_empty() {
            return None;
        }

        match self.last {
            Some(FlagInner {
                kind: Flag::Separator | Flag::Long(_) | Flag::Value,
                ..
            }) => None,
            Some(FlagInner {
                kind: Flag::Short(_),
                ..
            }) => {
                match self
                    .text
                    .chars()
                    .next()
                    .expect("internal error: empty strings are checked above")
                {
                    '=' => None,
                    ch => {
                        self.text = &self.text[ch.len_utf8()..];
                        self.last = Some(FlagInner {
                            kind: Flag::Short(ch),
                            value: self.text,
                        });

                        self.last
                    }
                }
            }
            None => {
                let (arg, next) = FlagInner::new(self.text);

                self.last = Some(arg);
                self.text = next;

                Some(arg)
            }
        }
    }
}

/// Parser for arguments.
///
/// # Examples
///
/// ```
/// # use storm::config::opts::{Flag, Parser};
///
/// let mut parser = Parser::new(["ls", "-lsh", "foo"].iter());
/// assert_eq!(parser.next(), Some(Flag::Value));
/// assert_eq!(parser.next(), Some(Flag::Short('l')));
/// assert_eq!(parser.next(), Some(Flag::Short('s')));
/// assert_eq!(parser.next(), Some(Flag::Short('h')));
/// assert_eq!(parser.value(Flag::Short('h')), Ok("foo"));
/// ```
#[derive(Debug)]
pub struct Parser<'arg, I, S>
where
    I: Iterator<Item = &'arg S>,
    S: AsRef<str> + ?Sized + 'arg,
{
    argv: I,
    arg: Option<Argument<'arg>>,
}
impl<'arg, I, S> Parser<'arg, I, S>
where
    I: Iterator<Item = &'arg S>,
    S: AsRef<str> + ?Sized + 'arg,
{
    pub const fn new(argv: I) -> Self {
        Self { argv, arg: None }
    }
}
impl<'arg, I, S> Parser<'arg, I, S>
where
    I: Iterator<Item = &'arg S>,
    S: AsRef<str> + ?Sized + 'arg,
{
    pub fn value(&mut self, flag: Flag<'arg>) -> Result<&'arg str, NoValueError<'arg>> {
        self.arg
            .and_then(|arg| arg.last)
            .and_then(|flag| flag.value())
            .inspect(|_| self.arg = None)
            .or_else(|| {
                self.advance().and_then(|flag| match flag.kind {
                    Flag::Value => Some(flag.value),
                    _ => None,
                })
            })
            .ok_or(NoValueError(flag))
    }

    fn advance(&mut self) -> Option<FlagInner<'arg>> {
        self.arg
            .insert(Argument::new(self.argv.next()?))
            .next()
            .or_else(|| self.advance())
    }
}
impl<'arg, I, S> Iterator for Parser<'arg, I, S>
where
    I: Iterator<Item = &'arg S>,
    S: AsRef<str> + ?Sized + 'arg,
{
    type Item = Flag<'arg>;

    fn next(&mut self) -> Option<Flag<'arg>> {
        match &mut self.arg {
            Some(flag) => flag
                .next()
                .map(|flag| flag.kind)
                .or_else(|| self.advance().map(|flag| flag.kind)),
            None => self.advance().map(|flag| flag.kind),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct NoValueError<'a>(Flag<'a>);
impl Display for NoValueError<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "flag `{}` is missing an argument", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags() {
        let mut flag = Argument::new("-Syuu");
        assert_eq!(
            flag.next(),
            Some(FlagInner {
                kind: Flag::Short('S'),
                value: "yuu"
            })
        );
        assert_eq!(
            flag.next(),
            Some(FlagInner {
                kind: Flag::Short('y'),
                value: "uu"
            })
        );
        assert_eq!(
            flag.next(),
            Some(FlagInner {
                kind: Flag::Short('u'),
                value: "u"
            })
        );
        assert_eq!(
            flag.next(),
            Some(FlagInner {
                kind: Flag::Short('u'),
                value: ""
            })
        );
        assert_eq!(flag.next(), None);

        let mut flag = Argument::new("-W=foo");
        assert_eq!(
            flag.next(),
            Some(FlagInner {
                kind: Flag::Short('W'),
                value: "=foo"
            })
        );
        assert_eq!(flag.next(), None);

        let mut flag = Argument::new("--help");
        assert_eq!(
            flag.next(),
            Some(FlagInner {
                kind: Flag::Long("help"),
                value: ""
            })
        );
        assert_eq!(flag.next(), None);

        let mut flag = Argument::new("--help=foo");
        assert_eq!(
            flag.next(),
            Some(FlagInner {
                kind: Flag::Long("help"),
                value: "foo"
            })
        );
        assert_eq!(flag.next(), None);

        let mut flag = Argument::new("help");
        assert_eq!(
            flag.next(),
            Some(FlagInner {
                kind: Flag::Value,
                value: "help"
            })
        );
        assert_eq!(flag.next(), None);

        let mut flag = Argument::new("--");
        assert_eq!(
            flag.next(),
            Some(FlagInner {
                kind: Flag::Separator,
                value: ""
            })
        );
        assert_eq!(flag.next(), None);
    }

    #[test]
    fn argument() {
        assert_eq!(
            Argument::new("-Wall").next().and_then(|arg| arg.value()),
            Some("all")
        );
        assert_eq!(Argument::new("-W").next().and_then(|arg| arg.value()), None);

        assert_eq!(
            Argument::new("-W=all").next().and_then(|arg| arg.value()),
            Some("all")
        );
        assert_eq!(
            Argument::new("-W=").next().and_then(|arg| arg.value()),
            None
        );

        assert_eq!(
            Argument::new("--foo=bar")
                .next()
                .and_then(|arg| arg.value()),
            Some("bar")
        );
        assert_eq!(
            Argument::new("--foo=").next().and_then(|arg| arg.value()),
            None
        );
    }

    #[test]
    fn top_level() {
        let mut parser = Parser::new(["-ctest"].into_iter());
        assert_eq!(parser.next(), Some(Flag::Short('c')));
        assert_eq!(parser.value(Flag::Short('c')), Ok("test"));
        assert_eq!(parser.next(), None);
    }
}
