use std::{borrow::Cow, ops::Range, rc::Rc};

/// String type that splits without extra allocations (will allocate once if the `Cow::Owned` needs
/// to shed excess capacity).
#[derive(Debug, PartialEq)]
pub enum SplitStr<'a> {
    Cow(Cow<'a, str>),
    Split { str: Rc<str>, range: Range<usize> },
}
impl<'a> From<&'a str> for SplitStr<'a> {
    fn from(str: &'a str) -> Self {
        Self::Cow(Cow::Borrowed(str))
    }
}
impl<'a> From<String> for SplitStr<'a> {
    fn from(str: String) -> Self {
        Self::Cow(Cow::Owned(str))
    }
}
impl AsRef<str> for SplitStr<'_> {
    fn as_ref(&self) -> &str {
        match self {
            Self::Cow(str) => &str,
            Self::Split { str, range } => &str[range.clone()], // ranges are fast to clone
        }
    }
}
impl SplitStr<'_> {
    pub fn split_at_checked(self, index: usize) -> Option<(Self, Self)> {
        match self {
            Self::Cow(Cow::Borrowed(str)) => str
                .split_at_checked(index)
                .map(|(l, r)| (l.into(), r.into())),
            Self::Cow(Cow::Owned(str)) => {
                if !str.is_char_boundary(index) {
                    return None;
                }

                let len = str.len();
                let rc: Rc<str> = Rc::from(str.into_boxed_str());
                Some((
                    Self::Split {
                        str: Rc::clone(&rc),
                        range: 0..index,
                    },
                    Self::Split {
                        str: rc,
                        range: index..len,
                    },
                ))
            }
            Self::Split { str, range } => {
                let index = range.start + index;
                if !range.contains(&index) || !str.is_char_boundary(index) {
                    return None;
                }

                Some((
                    Self::Split {
                        str: Rc::clone(&str),
                        range: range.start..index,
                    },
                    Self::Split {
                        str: str,
                        range: index..range.end,
                    },
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split() {
        // spliting [str]s
        let str = SplitStr::from("goodbye");

        let (good, bye) = str.split_at_checked(4).unwrap();
        assert_eq!(good.as_ref(), "good");
        assert_eq!(bye.as_ref(), "bye");

        // spliting [String]s
        let str = SplitStr::from("goodbye".to_string());
        let (good, bye) = str.split_at_checked(4).unwrap();
        assert_eq!(good.as_ref(), "good");
        drop(good);
        assert_eq!(bye.as_ref(), "bye");

        // spliting [SplitStr::Split]s
        let (b, ye) = bye.split_at_checked(1).unwrap();
        assert_eq!(b.as_ref(), "b");
        assert_eq!(ye.as_ref(), "ye");
    }
}
