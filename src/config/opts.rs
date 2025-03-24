use {
    crate::recursion::Recursion,
    std::{
        fmt::{self, Display, Formatter},
        marker::PhantomData,
    },
};

#[derive(Clone, Debug, PartialEq)]
pub struct Arg<'a> {
    last_flag_kind: Option<FlagKind>,
    next: &'a str,
}
impl<'a> Arg<'a> {
    fn value(mut self) -> Option<&'a str> {
        self.last_flag_kind.and_then(|_| {
            let offset = match self.next.chars().next()? {
                '=' => 1,
                _ => 0,
            };

            self.next = &self.next[offset..];
            Some(self.next)
        })
    }
}
impl<'a> Iterator for Arg<'a> {
    type Item = Result<Flag<'a>, ArgError>;

    fn next(&mut self) -> Option<Result<Flag<'a>, ArgError>> {
        match self.last_flag_kind {
            Some(FlagKind::Long) => None,
            Some(FlagKind::Short) => match self.next.chars().next()? {
                '=' => None,
                ch => {
                    self.next = &self.next[ch.len_utf8()..];

                    Some(Ok(Flag::Short(ch)))
                }
            },
            None => match self.next.as_bytes() {
                [b'-', b'-'] => Some(Err(ArgError::Separator)),
                [b'-', b'-', ..] => {
                    self.next = &self.next[2..];
                    self.last_flag_kind = Some(FlagKind::Long);

                    if let Some((split, _)) = self.next.char_indices().find(|(_, ch)| *ch == '=') {
                        let (flag, next) = self.next.split_at(split);
                        self.next = next;

                        Some(Ok(Flag::Long(flag)))
                    } else {
                        let flag = Flag::Long(self.next);
                        self.next = "";

                        Some(Ok(flag))
                    }
                }
                // Having 1 ascii character and 1 random byte would make it always have a full char.
                // Also not ub since this is never read.
                [b'-', _, ..] => {
                    self.next = &self.next[1..];

                    let flag = self.next.chars().next().unwrap();
                    self.next = &self.next[flag.len_utf8()..];

                    self.last_flag_kind = Some(FlagKind::Short);
                    Some(Ok(Flag::Short(flag)))
                }
                [] => None,
                _ => Some(Err(ArgError::Value)),
            },
        }
    }
}
impl<'a> From<&'a str> for Arg<'a> {
    fn from(str: &'a str) -> Self {
        Self {
            last_flag_kind: None,
            next: str,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ArgError {
    Separator,
    Value,
}

pub struct Argv<'a, I, E>
where
    I: Iterator<Item = Result<&'a str, E>>,
{
    iter: I,
    last: Option<Arg<'a>>,
    passed_separator: bool,
    _marker: PhantomData<E>,
}
impl<'a, I, O, E> From<I> for Argv<'a, O, E>
where
    I: IntoIterator<Item = Result<&'a str, E>, IntoIter = O>,
    O: Iterator<Item = Result<&'a str, E>>,
{
    fn from(iter: I) -> Self {
        Self {
            iter: iter.into_iter(),
            last: None,
            passed_separator: false,
            _marker: PhantomData,
        }
    }
}
impl<'a, I, E> Argv<'a, I, E>
where
    I: Iterator<Item = Result<&'a str, E>>,
{
    /// Returns none if there are no more arguments.
    fn last_or_next(&mut self) -> Option<Result<&mut Arg<'a>, E>> {
        if self.last.is_none() {
            Some(Ok(self.last.insert(match self.iter.next()? {
                Ok(arg) => Arg::from(arg),
                Err(err) => return Some(Err(err)),
            })))
        } else {
            self.last.as_mut().map(Ok)
        }
    }

    /// Get a value if it exists.
    pub fn value(&mut self) -> Option<Result<&'a str, E>> {
        self.last.take().and_then(Arg::value).map(Ok).or_else(|| {
            let value = match self.iter.next()? {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };

            if let Err(ArgError::Value) = Arg::from(value).next()? {
                Some(Ok(value))
            } else {
                self.last = Some(Arg::from(value));
                None
            }
        })
    }
}
impl<'a, I, E> Iterator for Argv<'a, I, E>
where
    I: Iterator<Item = Result<&'a str, E>>,
{
    type Item = Result<Flag<'a>, E>;

    fn next(&mut self) -> Option<Result<Flag<'a>, E>> {
        Recursion::start(self, |s| {
            if s.passed_separator {
                return Recursion::End(None);
            }

            let arg = match s.last_or_next() {
                Some(Ok(arg)) => arg,
                Some(Err(err)) => return Recursion::End(Some(Err(err))),
                None => return Recursion::End(None),
            };

            match arg.next().transpose() {
                Ok(flag @ Some(_)) => Recursion::End(flag.map(Ok)),
                Ok(None) | Err(ArgError::Value) => {
                    s.last = None;
                    Recursion::Continue(s)
                }
                Err(ArgError::Separator) => {
                    s.passed_separator = true;
                    Recursion::End(None)
                }
            }
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Flag<'a> {
    /// Arguments that start with `--`
    Long(&'a str),
    /// Arguments that start with `-`
    Short(char),
}
impl Display for Flag<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Long(flag) => write!(f, "--{}", flag),
            Self::Short(flag) => write!(f, "-{}", flag),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
/// [Flag] without its contents
enum FlagKind {
    Long,
    Short,
}
impl<'a> From<&Flag<'a>> for FlagKind {
    fn from(flag: &Flag<'a>) -> Self {
        match flag {
            Flag::Long(_) => Self::Long,
            Flag::Short(_) => Self::Short,
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, std::convert::Infallible};

    #[test]
    fn flag_inner_init() {
        [
            (
                "--foo=bar",
                Some(Ok(Flag::Long("foo"))),
                Some(Arg {
                    last_flag_kind: Some(FlagKind::Long),
                    next: "=bar".into(),
                }),
            ),
            (
                "-foo=bar",
                Some(Ok(Flag::Short('f'))),
                Some(Arg {
                    last_flag_kind: Some(FlagKind::Short),
                    next: "oo=bar".into(),
                }),
            ),
            ("", None, None),
            ("--", Some(Err(ArgError::Separator)), None),
            ("-", Some(Err(ArgError::Value)), None),
            ("foo bar", Some(Err(ArgError::Value)), None),
        ]
        .into_iter()
        .for_each(|(input, output, next_state)| {
            let mut flag = Arg::from(input);
            assert_eq!(flag.next(), output);

            if let Some(next_state) = next_state {
                assert_eq!(flag, next_state);
            }
        });
    }
    #[test]
    fn flag_inner_collect() {
        [
            (
                "-foobar",
                &[
                    Flag::Short('f'),
                    Flag::Short('o'),
                    Flag::Short('o'),
                    Flag::Short('b'),
                    Flag::Short('a'),
                    Flag::Short('r'),
                ] as &[_],
            ),
            (
                "-foo=bar",
                &[Flag::Short('f'), Flag::Short('o'), Flag::Short('o')],
            ),
            ("--foo=bar", &[Flag::Long("foo")]),
            ("--foo", &[Flag::Long("foo")]),
        ]
        .into_iter()
        .for_each(|(input, expected)| {
            Arg::from(input)
                .enumerate()
                .for_each(|(i, line)| assert_eq!(line.as_ref(), Ok(&expected[i])));
        });
    }
    #[test]
    fn flag_inner_values() {
        [
            ("--foo=bar", 1, "bar"),
            ("-Wall", 1, "all"),
            ("-W=all", 1, "all"),
            ("-Syuu=foo", 4, "foo"),
        ]
        .into_iter()
        .for_each(|(input, nth, value)| {
            let mut arg = Arg::from(input);
            (0..nth).map(|_| arg.next()).for_each(drop);

            assert_eq!(arg.value(), Some(value));
        })
    }

    #[test]
    fn argv_collect() {
        [
            (
                &["--foo", "-lsh"] as &[_],
                &[
                    Flag::Long("foo"),
                    Flag::Short('l'),
                    Flag::Short('s'),
                    Flag::Short('h'),
                ] as &[_],
            ),
            (
                &["--foo", "-Syuu", "--", "-Wall"],
                &[
                    Flag::Long("foo"),
                    Flag::Short('S'),
                    Flag::Short('y'),
                    Flag::Short('u'),
                    Flag::Short('u'),
                ],
            ),
        ]
        .into_iter()
        .for_each(|(argv, expected)| {
            Argv::from(argv.iter().copied().map(Ok::<_, Infallible>))
                .enumerate()
                .for_each(|(i, flag)| assert_eq!(flag, Ok(expected[i])));
        })
    }
    #[test]
    fn argv_value() {
        [
            (
                &["--foo", "bar", "-lsh"] as &[_],
                1,
                "bar",
                &[Flag::Short('l'), Flag::Short('s'), Flag::Short('h')] as &[_],
            ),
            (
                &["--foo=bar", "-lsh"] as &[_],
                1,
                "bar",
                &[Flag::Short('l'), Flag::Short('s'), Flag::Short('h')],
            ),
            (&["--foo=bar", "-lsh"] as &[_], 2, "sh", &[]),
        ]
        .into_iter()
        .for_each(|(input, nth, expected_value, expected_flags)| {
            let mut argv = Argv::from(input.iter().copied().map(Ok::<_, Infallible>));
            (0..nth).map(|_| argv.next()).for_each(drop);
            assert_eq!(argv.value(), Some(Ok(expected_value)));

            argv.enumerate()
                .for_each(|(i, flag)| assert_eq!(flag, Ok(expected_flags[i])));
        })
    }
}
