#![no_std]

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Flag<'arg> {
    Separator,
    Value,
    Short(char),
    Long(&'arg str),
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
        } else if arg.starts_with("--") {
            let next = &arg[2..];

            let (identifier, value) = next
                .char_indices()
                .find(|(_, ch)| *ch == '=')
                .and_then(|(i, _)| Some((&next[..i], &next[i.checked_add(1)?..])))
                .unwrap_or((next, ""));

            (
                FlagInner {
                    kind: Flag::Long(identifier),
                    value: value,
                },
                next,
            )
        } else if arg.starts_with('-') && arg.len() >= 2 {
            let mut arg = arg.chars();
            arg.next();

            (
                FlagInner {
                    kind: Flag::Short(arg.next().expect(
                        "the check above should ensure that there is always more than 1 characters",
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
                .get(self.value.starts_with('=').then_some(1).unwrap_or(0)..),
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
                kind: Flag::Separator
                | Flag::Long(_)
                | Flag::Value,
                ..
            }) => None,
            Some(FlagInner {
                kind: Flag::Short(_),
                ..
            }) => {
                match self.text.chars()
                    .next()
                    .expect("checking whether or not text is empty is done above, which should ensure that this is unreachable") {
                    '=' => None,
                    ch => {
                        self.text = &self.text[ch.len_utf8()..];
                        self.last = Some(FlagInner{ kind: Flag::Short(ch), value: self.text });

                        self.last
                    },
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
/// # use opts::{Flag, Parser};
///
/// let mut parser = Parser::new(["ls", "-lsh", "foo"].iter());
/// assert_eq!(parser.next(), Some(Flag::Value));
/// assert_eq!(parser.next(), Some(Flag::Short('l')));
/// assert_eq!(parser.next(), Some(Flag::Short('s')));
/// assert_eq!(parser.next(), Some(Flag::Short('h')));
/// assert_eq!(parser.value(), Ok("foo"));
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
    pub fn value(&mut self) -> Result<&'arg str, NoValueError> {
        self.arg
            .and_then(|arg| arg.last)
            .and_then(|flag| flag.value())
            .or_else(|| {
                self.advance().and_then(|flag| match flag.kind {
                    Flag::Value => Some(flag.value),
                    _ => None,
                })
            })
            .ok_or(NoValueError)
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
pub struct NoValueError;

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
}
