use {
    crate::{recursion::Recursion, str::copy_str::CopyStr},
    std::{
        fmt::{self, Display, Formatter},
        mem::replace,
    },
};

#[derive(Clone, Debug, PartialEq)]
pub struct Arg<'a> {
    last_flag_kind: Option<FlagKind>,
    next: CopyStr<'a>,
}
impl<'a> Arg<'a> {
    fn value(mut self) -> Option<CopyStr<'a>> {
        self.last_flag_kind.and_then(|_| {
            let offset = match self.next.as_ref().chars().next()? {
                '=' => 1,
                _ => 0,
            };

            self.next.cut_at(offset);
            Some(self.next)
        })
    }
}
impl<'a> Iterator for Arg<'a> {
    type Item = Result<Flag<'a>, ArgError>;

    fn next(&mut self) -> Option<Result<Flag<'a>, ArgError>> {
        match self.last_flag_kind {
            Some(FlagKind::Long) => None,
            Some(FlagKind::Short) => match self.next.as_ref().chars().next()? {
                '=' => None,
                ch => {
                    self.next.cut_at(ch.len_utf8());

                    Some(Ok(Flag::Short(ch)))
                }
            },
            None => match self.next.as_ref().as_bytes() {
                [b'-', b'-'] => Some(Err(ArgError::Separator)),
                [b'-', b'-', ..] => {
                    self.next.cut_at(2);
                    self.last_flag_kind = Some(FlagKind::Long);

                    if let Some((split, _)) =
                        self.next.as_ref().char_indices().find(|(_, ch)| *ch == '=')
                    {
                        let next = self.next.split_off(split);

                        Some(Ok(Flag::Long(replace(&mut self.next, next))))
                    } else {
                        Some(Ok(Flag::Long(replace(&mut self.next, CopyStr::from("")))))
                    }
                }
                // Having 1 ascii character and 1 random byte would make it always have a full char.
                // Also not ub since this is never read.
                [b'-', _, ..] => {
                    self.next.cut_at(1);

                    let flag = self.next.as_ref().chars().next().unwrap();
                    self.next.cut_at(flag.len_utf8());

                    self.last_flag_kind = Some(FlagKind::Short);
                    Some(Ok(Flag::Short(flag)))
                }
                [] => None,
                _ => Some(Err(ArgError::Value)),
            },
        }
    }
}
impl<'a> From<CopyStr<'a>> for Arg<'a> {
    fn from(str: CopyStr<'a>) -> Self {
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

pub struct Argv<'a, I>
where
    I: Iterator<Item = CopyStr<'a>>,
{
    iter: I,
    last: Option<Arg<'a>>,
    passed_separator: bool,
}
impl<'a, I, O> From<I> for Argv<'a, O>
where
    I: IntoIterator<Item = CopyStr<'a>, IntoIter = O>,
    O: Iterator<Item = CopyStr<'a>>,
{
    fn from(iter: I) -> Self {
        Self {
            iter: iter.into_iter(),
            last: None,
            passed_separator: false,
        }
    }
}
impl<'a, I> Argv<'a, I>
where
    I: Iterator<Item = CopyStr<'a>>,
{
    /// Returns none if there are no more arguments.
    fn last_or_next(&mut self) -> Option<&mut Arg<'a>> {
        if self.last.is_none() {
            Some(self.last.insert(self.iter.next().map(Arg::from)?))
        } else {
            self.last.as_mut()
        }
    }

    /// Get a value if it exists.
    pub fn value(&mut self) -> Option<CopyStr<'a>> {
        self.last.take().and_then(Arg::value).or_else(|| {
            let value = self.iter.next()?;

            if let Err(ArgError::Value) = Arg::from(CopyStr::from(value.as_ref())).next()? {
                Some(value)
            } else {
                self.last = Some(Arg::from(value));
                None
            }
        })
    }
}
impl<'a, I> Iterator for Argv<'a, I>
where
    I: Iterator<Item = CopyStr<'a>>,
{
    type Item = Flag<'a>;

    fn next(&mut self) -> Option<Flag<'a>> {
        Recursion::start(self, |s| {
            if s.passed_separator {
                return Recursion::End(None);
            }

            let arg = match s.last_or_next() {
                Some(arg) => arg,
                None => return Recursion::End(None),
            };

            match arg.next().transpose() {
                Ok(flag @ Some(_)) => Recursion::End(flag),
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

#[derive(Clone, Debug, PartialEq)]
pub enum Flag<'a> {
    /// Arguments that start with `--`
    Long(CopyStr<'a>),
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
    use super::*;

    #[test]
    fn flag_inner_init() {
        [
            (
                "--foo=bar",
                Some(Ok(Flag::Long("foo".into()))),
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
            let mut flag = Arg::from(CopyStr::from(input));
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
            ("--foo=bar", &[Flag::Long("foo".into())]),
            ("--foo", &[Flag::Long("foo".into())]),
        ]
        .into_iter()
        .for_each(|(input, expected)| {
            assert_eq!(
                Arg::from(CopyStr::from(input))
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap()
                    .as_slice(),
                expected
            );
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
            let mut arg = Arg::from(CopyStr::from(input));
            (0..nth).map(|_| arg.next()).for_each(drop);

            assert_eq!(arg.value(), Some(CopyStr::from(value)));
        })
    }

    #[test]
    fn argv_collect() {
        [
            (
                &["--foo", "-lsh"] as &[_],
                &[
                    Flag::Long("foo".into()),
                    Flag::Short('l'),
                    Flag::Short('s'),
                    Flag::Short('h'),
                ] as &[_],
            ),
            (
                &["--foo", "-Syuu", "--", "-Wall"],
                &[
                    Flag::Long("foo".into()),
                    Flag::Short('S'),
                    Flag::Short('y'),
                    Flag::Short('u'),
                    Flag::Short('u'),
                ],
            ),
        ]
        .into_iter()
        .for_each(|(argv, expected)| {
            assert_eq!(
                Argv::from(argv.iter().copied().map(CopyStr::from))
                    .collect::<Vec<_>>()
                    .as_slice(),
                expected,
            )
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
        .for_each(|(input, nth, expected_value, expected_collect)| {
            let mut argv = Argv::from(input.iter().copied().map(CopyStr::from));
            (0..nth).map(|_| argv.next()).for_each(drop);
            assert_eq!(argv.value(), Some(CopyStr::from(expected_value)));
            assert_eq!(argv.collect::<Vec<_>>().as_slice(), expected_collect);
        })
    }
}
