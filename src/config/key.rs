use {
    crate::split_str::SplitStr,
    enum_map::{Enum, EnumMap},
    std::{borrow::Cow, collections::VecDeque},
};

#[derive(Enum)]
pub enum KeyAction {
    Quit,
}

#[derive(Debug, PartialEq)]
pub struct Key<'a>(SplitStr<'a>, KeyModifiers);
#[derive(Debug, PartialEq)]
pub struct KeySequence<'a>(VecDeque<Key<'a>>);
impl<'a> FromIterator<(Cow<'a, str>, KeyModifiers)> for KeySequence<'a> {
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = (Cow<'a, str>, KeyModifiers)>,
    {
        let mut iter = iter.into_iter().peekable();
        let mut deque = VecDeque::with_capacity(iter.size_hint().0);

        while let Some((mut next_str, next_mods)) = iter.next() {
            while let Some((peeked_str, _)) =
                iter.next_if(|(_, peeked_mods)| next_mods == *peeked_mods)
            {
                next_str.to_mut().push_str(&peeked_str);
            }

            deque.push_back(Key(SplitStr::Cow(next_str), next_mods));
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
    fn key_sequence_flatten() {
        assert_eq!(
            KeySequence::from_iter([
                ("foo".into(), KeyModifiers::from_fn(|_| false)),
                ("bar".into(), KeyModifiers::from_fn(|_| false)),
                ("foo".into(), KeyModifiers::from_fn(|_| true)),
                ("bar".into(), KeyModifiers::from_fn(|_| true)),
            ]),
            KeySequence::from_iter([
                ("foobar".into(), KeyModifiers::from_fn(|_| false)),
                ("foobar".into(), KeyModifiers::from_fn(|_| true)),
            ])
        );
    }
}
