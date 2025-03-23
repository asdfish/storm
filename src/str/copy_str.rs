//! String type with somewhat fast copies and splits

use std::{
    borrow::Cow,
    fmt::{self, Display, Formatter},
    ops::{Bound, Deref, Range, RangeBounds},
    rc::Rc,
};

#[derive(Clone, Debug, Default)]
pub struct CopyStr<'a> {
    buffer: CopyStrBuffer<'a>,
    bounds: Range<usize>,
}
impl<'a> CopyStr<'a> {
    pub fn get<R>(&self, range: R) -> Option<Self>
    where
        R: RangeBounds<usize>,
    {
        let start = match range.start_bound() {
            Bound::Included(from) => *from,
            _ => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(from) => from.checked_add(1)?,
            Bound::Excluded(from) => *from,
            Bound::Unbounded => self.bounds.end,
        };

        if [start, end]
            .into_iter()
            .any(|i| !self.buffer.is_char_boundary(i))
        {
            None
        } else {
            Some(Self {
                buffer: self.buffer.clone(),
                bounds: start..end,
            })
        }
    }

    /// # Safety
    ///
    /// Will panic if `at` is not a valid character boundary
    pub fn cut_at(&mut self, at: usize) {
        assert!(self.buffer.is_char_boundary(at));
        self.bounds.start += at;
    }
    pub fn cut_at_checked(mut self, at: usize) -> Option<Self> {
        if self.buffer.is_char_boundary(at) {
            self.bounds.start += at;
            Some(self)
        } else {
            None
        }
    }

    /// # Safety
    ///
    /// Ensure that `at` is a valid character boundary
    pub unsafe fn cut_at_unchecked(&mut self, at: usize) {
        self.bounds.start += at;
    }

    /// # Panics
    ///
    /// Will panic if `at` is not a character boundary.
    pub fn split_at(self, at: usize) -> (Self, Self) {
        assert!(self.buffer.is_char_boundary(self.bounds.start + at));

        unsafe { self.split_at_unchecked(at) }
    }
    pub fn split_at_checked(self, at: usize) -> Option<(Self, Self)> {
        if self.buffer.is_char_boundary(self.bounds.start + at) {
            Some(unsafe { self.split_at_unchecked(at) })
        } else {
            None
        }
    }

    /// # Safety
    ///
    /// Ensure that `at` is a valid character boundary
    pub unsafe fn split_at_unchecked(self, at: usize) -> (Self, Self) {
        (
            Self {
                buffer: self.buffer.clone(),
                bounds: Range {
                    start: self.bounds.start,
                    end: self.bounds.start + at,
                },
            },
            Self {
                buffer: self.buffer,
                bounds: Range {
                    start: self.bounds.start + at,
                    end: self.bounds.end,
                },
            },
        )
    }

    pub fn split_off(&mut self, at: usize) -> Self {
        assert!(self.buffer.is_char_boundary(at));
        let split = self.bounds.start + at;

        let out = Self {
            buffer: self.buffer.clone(),
            bounds: Range {
                start: split,
                end: self.bounds.end,
            },
        };
        self.bounds.end = split;

        out
    }
}
impl Display for CopyStr<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}
impl PartialEq for CopyStr<'_> {
    fn eq(&self, rhs: &Self) -> bool {
        self.as_ref() == rhs.as_ref()
    }
}
impl AsRef<str> for CopyStr<'_> {
    fn as_ref(&self) -> &str {
        &self.buffer[self.bounds.clone()]
    }
}
impl<'a> From<Cow<'a, str>> for CopyStr<'a> {
    fn from(str: Cow<'a, str>) -> Self {
        match str {
            Cow::Borrowed(str) => Self::from(str),
            Cow::Owned(str) => Self::from(Rc::from(str.into_boxed_str())),
        }
    }
}
impl From<Rc<str>> for CopyStr<'_> {
    fn from(buffer: Rc<str>) -> Self {
        Self {
            bounds: 0..buffer.len(),
            buffer: CopyStrBuffer::Rc(buffer),
        }
    }
}
impl<'a> From<&'a str> for CopyStr<'a> {
    fn from(buffer: &'a str) -> Self {
        Self {
            bounds: 0..buffer.len(),
            buffer: CopyStrBuffer::Ref(buffer),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum CopyStrBuffer<'a> {
    Rc(Rc<str>),
    Ref(&'a str),
}
impl AsRef<str> for CopyStrBuffer<'_> {
    fn as_ref(&self) -> &str {
        match self {
            Self::Rc(rc) => rc,
            Self::Ref(str) => str,
        }
    }
}
impl Deref for CopyStrBuffer<'_> {
    type Target = str;
    fn deref(&self) -> &str {
        self.as_ref()
    }
}
impl Default for CopyStrBuffer<'_> {
    fn default() -> Self {
        Self::Ref("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_str_get() {
        assert_eq!(CopyStr::from("foo").get(0..1).unwrap().as_ref(), "f");
        assert_eq!(CopyStr::from("foo").get(..1).unwrap().as_ref(), "f");
        assert_eq!(CopyStr::from("foo").get(0..=1).unwrap().as_ref(), "fo");
        assert_eq!(CopyStr::from("foo").get(..=1).unwrap().as_ref(), "fo");
        assert_eq!(CopyStr::from("foo").get(..).unwrap().as_ref(), "foo");
    }

    #[test]
    fn copy_str_from() {
        assert_eq!(CopyStr::from("foo").as_ref(), "foo");
        assert_eq!(
            CopyStr::from(Cow::Owned(String::from("foo"))).as_ref(),
            "foo"
        );
        assert_eq!(CopyStr::from(Rc::from("foo")).as_ref(), "foo");
    }

    #[test]
    fn copy_str_split() {
        let (l, r) = CopyStr::from("foo").split_at(2);
        assert_eq!(l.as_ref(), "fo");
        assert_eq!(r.as_ref(), "o");
    }

    #[test]
    fn copy_str_split_off() {
        let mut l = CopyStr::from("foo");
        let r = l.split_off(1);

        assert_eq!(l.as_ref(), "f");
        assert_eq!(r.as_ref(), "oo");
    }
}
