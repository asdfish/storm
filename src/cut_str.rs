use std::{borrow::Cow, ops::Range, rc::Rc};

/// String type that splits without extra allocations (will allocate once if the `Cow::Owned` needs
/// to shed excess capacity).
#[derive(Debug, PartialEq)]
pub enum CutStr<'a> {
    Cow(Cow<'a, str>),
    Cut { str: String, head: usize },
}
impl<'a> From<&'a str> for CutStr<'a> {
    fn from(str: &'a str) -> Self {
        Self::Cow(Cow::Borrowed(str))
    }
}
impl<'a> From<String> for CutStr<'a> {
    fn from(str: String) -> Self {
        Self::Cow(Cow::Owned(str))
    }
}
impl AsRef<str> for CutStr<'_> {
    fn as_ref(&self) -> &str {
        match self {
            Self::Cow(str) => str,
            Self::Cut { str, head, } => &str[*head..],
        }
    }
}
impl CutStr<'_> {
    pub fn cut_checked(self, index: usize) -> Option<Self> {
        match self {
            Self::Cow(Cow::Borrowed(str)) => str
                .get(index..)
            .map(CutStr::from),
            Self::Cow(Cow::Owned(str)) => {
                if !str.is_char_boundary(index) {
                    return None;
                }

                let len = str.len();
                Some(Self::Cut {
                    str,
                    head: index,
                })
            }
            Self::Cut { str, head } => {
                let head = head + index;
                str.is_char_boundary(head)
                    .then(|| Self::Cut {
                        str,
                        head,
                    })
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
        let str = CutStr::from("goodbye");

        let (good, bye) = str.split_at_checked(4).unwrap();
        assert_eq!(good.as_ref(), "good");
        assert_eq!(bye.as_ref(), "bye");

        // spliting [String]s
        let str = CutStr::from("goodbye".to_string());
        let (good, bye) = str.split_at_checked(4).unwrap();
        assert_eq!(good.as_ref(), "good");
        drop(good);
        assert_eq!(bye.as_ref(), "bye");

        // spliting [CutStr::Split]s
        let (b, ye) = bye.split_at_checked(1).unwrap();
        assert_eq!(b.as_ref(), "b");
        assert_eq!(ye.as_ref(), "ye");
    }
}
