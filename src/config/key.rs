use {
    crate::bomb::Bomb,
    enum_map::{Enum, EnumMap},
    std::{borrow::Cow, collections::VecDeque, mem::{ManuallyDrop, take}, ops::Deref},
};

/// [std::borrow::Cow] with the capability to be boxed.
#[derive(Debug, PartialEq)]
pub enum OwnedStr<'a> {
    Cow(Cow<'a, str>),
    /// This needs to be an `Option<Box<str>>` because empty strings are not zsts and will allocate when using [Option::take].
    Box(Option<Box<str>>),
}
impl<'a> From<&'a str> for OwnedStr<'a> {
    fn from(str: &'a str) -> Self {
        Self::Cow(Cow::Borrowed(str))
    }
}
impl AsRef<str> for OwnedStr<'_> {
    fn as_ref(&self) -> &str {
        match self {
            Self::Cow(str) => &str,
            Self::Box(Some(str)) => &str,
            Self::Box(None) => "",
        }
    }
}
impl OwnedStr<'_> {
    pub fn split_at_checked(self, index: usize) -> Option<(Self, Self)> {
        let mut str = Bomb::new(Box::<str>::leak(match self {
            Self::Cow(Cow::Borrowed(str)) => Box::from(str),
            Self::Cow(Cow::Owned(str)) => str.into_boxed_str(),
            Self::Box(Some(str)) => str,
            Self::Box(None) => Default::default(),
        }), |str| {
            let _ = unsafe { Box::from_raw(*str as *mut _) };
        });

        let (left, right) = str.split_at_mut_checked(index)?;
        // SAFETY: since these are ManuallyDrop, a panic below wouldn't cause a double free
        let left = ManuallyDrop::new(unsafe { Box::from_raw(left as *mut str) });
        let right = ManuallyDrop::new(unsafe { Box::from_raw(right as *mut str) });
        str.diffuse();

        Some((Self::Box(Some(ManuallyDrop::into_inner(left))), Self::Box(Some(ManuallyDrop::into_inner(right)))))
    }

    pub fn to_mut(&mut self) -> &mut String {
        match self {
            Self::Cow(str) => str.to_mut(),
            Self::Box(str) => {
                *self = Self::Cow(Cow::Owned(str.take()
                    .map(<Box<str> as Into<String>>::into)
                    .unwrap_or_default()));
                match self {
                    Self::Cow(Cow::Owned(str)) => str,
                    _ => unreachable!("internal error: setting `self` to an `Self::Cow(Cow::Owned(_))` is done above"),
                }
            }
        }
    }
}

#[derive(Enum)]
pub enum KeyAction {
    Quit,
}

#[derive(Debug, PartialEq)]
pub struct Key<'a>(OwnedStr<'a>, KeyModifiers);
#[derive(Debug, PartialEq)]
pub struct KeySequence<'a>(VecDeque<Key<'a>>);
impl<'a> FromIterator<Key<'a>> for KeySequence<'a> {
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = Key<'a>>,
    {
        let mut iter = iter.into_iter().peekable();
        let mut deque = VecDeque::with_capacity(iter.size_hint().0);

        while let Some(mut next) = iter.next() {
            while let Some(Key(key, _)) = iter.next_if(|Key(_, modifiers)| next.1.eq(modifiers)) {
                next.0.to_mut().push_str(key.as_ref());
            }

            deque.push_back(next);
        }

        Self(deque)
    }
}

#[derive(Clone, Copy, Debug, Enum)]
/// The possible modifier keys from a key press.
///
/// Does not distinguish between left and right variants.
pub enum KeyModifier {
    /// AKA meta key.
    Alt,
    Control,
    Shift,
    /// Logo/windows key.
    Super,
}

pub type KeyModifiers = EnumMap<KeyModifier, bool>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owned_str_split() {
        let str = OwnedStr::from("goodbye");

        let (good, bye) = str.split_at_checked(4).unwrap();
        assert_eq!(good.as_ref(), "good");
        drop(good);
        assert_eq!(bye.as_ref(), "bye");
    }

    #[test]
    fn owned_str_to_mut_edgecase() {
        // test to see no panic
        OwnedStr::Box(None).to_mut();
        OwnedStr::Box(Some(Box::from(""))).to_mut();
    }

    #[test]
    fn key_sequence_flatten() {
        assert_eq!(
            KeySequence::from_iter([
                Key("foo".into(), KeyModifiers::from_fn(|_| false)),
                Key("bar".into(), KeyModifiers::from_fn(|_| false)),
                Key("foo".into(), KeyModifiers::from_fn(|_| true)),
                Key("bar".into(), KeyModifiers::from_fn(|_| true)),
            ]),
            KeySequence::from_iter([
                Key("foobar".into(), KeyModifiers::from_fn(|_| false)),
                Key("foobar".into(), KeyModifiers::from_fn(|_| true)),
            ])
        );
    }
}
