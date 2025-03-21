use {
    enum_map::{Enum, EnumMap},
    smallvec::SmallVec,
    std::{borrow::Cow, ops::Not},
};

#[derive(Enum)]
pub enum KeyAction {
    Quit,
}

#[derive(Debug, PartialEq)]
pub struct Key<'a>(Cow<'a, str>, KeyModifiers);

#[derive(Debug, PartialEq)]
/// The keys *never* contain the same modifiers while being chained.
pub struct KeySequence<'a>(SmallVec<[Key<'a>; 4]>);
impl KeySequence<'_> {
    pub fn new() -> Self {
        Self(SmallVec::new())
    }
    /// Allocate `n` elements in advance
    pub fn reserve(&mut self, n: usize) {
        self.0.reserve(n);
    }
    /// Shed excess capacity
    pub fn shrink_to_fit(&mut self) {
        self.0.shrink_to_fit();
    }
    /// Create `self` with `n` elements in advance
    pub fn with_capacity(cap: usize) -> Self {
        Self(SmallVec::with_capacity(cap))
    }
}
impl<'a> KeySequence<'a> {
    /// Add a new key or append to the current tail if they share modifiers
    pub fn push(&mut self, key: Key<'a>) {
        match self.0.last_mut() {
            Some(Key(text, mods)) if key.1.eq(mods) => {
                text.to_mut().push_str(&key.0);
            }
            _ => self.0.push(key),
        }
    }

    /// Whether or not `self` contains the key sequence described in other
    pub fn contains<'b>(&self, other: &KeySequence<'b>) -> bool {
        self.0.iter().zip(other.0.iter()).any(|(l, r)| l != r).not()
    }
}
impl<'a> Extend<Key<'a>> for KeySequence<'a> {
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = Key<'a>>,
    {
        let iter = iter.into_iter();
        self.0.reserve(iter.size_hint().0);
        iter.for_each(|key| self.push(key));
    }
}
impl<'a> FromIterator<Key<'a>> for KeySequence<'a> {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = Key<'a>>,
    {
        let mut output = Self::new();
        output.extend(iter);

        output
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
    fn key_sequence_contains() {
        assert!(
            KeySequence::from_iter([
                Key("foo".into(), KeyModifiers::from_fn(|_| false)),
                Key("bar".into(), KeyModifiers::from_fn(|_| true)),
            ])
            .contains(&KeySequence::from_iter([Key(
                "foo".into(),
                KeyModifiers::from_fn(|_| false)
            )]))
        );
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
