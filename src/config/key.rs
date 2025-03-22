use {
    enum_map::{Enum, EnumMap},
    smallvec::SmallVec,
    std::{
        borrow::Cow,
        fmt::{self, Display, Formatter},
        ops::Not,
    },
};

pub trait Parser<'a>: Sized {
    fn parse(_: &'a str) -> Result<(Self, &'a str), ParserError<'a>>;
}

#[derive(Enum)]
pub enum KeyAction {
    Quit,
}

#[derive(Debug, PartialEq)]
/// Represent a key press
pub struct Key<'a> {
    /// The modifiers that are active during
    mods: KeyModifiers,
    kind: KeyKind<'a>,
}
impl Display for Key<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match &self.kind {
            KeyKind::Invisible(key) => write!(f, "{}{}", self.mods, key),
            KeyKind::Visible(keys) => keys
                .chars()
                .try_for_each(|key| write!(f, "{}{}", self.mods, key)),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum KeyKind<'a> {
    /// Keys that cannot be represented using text (such as `F1`, `PageUp`, ..)
    Invisible(InvisibleKey),
    /// Keys that can be represented using text (such as 'a', 'A', 'b', ..)
    Visible(Cow<'a, str>),
}
impl Display for KeyKind<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invisible(key) => write!(f, "{}", key),
            Self::Visible(key) => key.chars().try_for_each(|ch| match ch {
                'M' | 'C' | 'S' | 'L' | '<' | '>' => write!(f, "\\{}", ch),
                ch => write!(f, "{}", ch),
            }),
        }
    }
}
impl<'a> From<Cow<'a, str>> for KeyKind<'a> {
    fn from(key: Cow<'a, str>) -> Self {
        Self::Visible(key)
    }
}
impl<'a> From<&'a str> for KeyKind<'a> {
    fn from(key: &'a str) -> Self {
        Self::Visible(key.into())
    }
}
impl<'a> Parser<'a> for KeyKind<'a> {
    fn parse(input: &'a str) -> Result<(Self, &'a str), ParserError<'a>> {
        if input.starts_with("<") {
            InvisibleKey::parse(input)
                .map(|(key, out)| (Self::Invisible(key), out))
        } else {
            let mut out = Cow::Borrowed("");
            let mut chars = input.char_indices();

            todo!()
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum InvisibleKey {
    /// Function keys
    F(u8),
    PageUp,
    PageDown,
}
impl Display for InvisibleKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "<")?;

        match self {
            Self::F(n) => write!(f, "F{n}"),
            Self::PageUp => write!(f, "PG-UP"),
            Self::PageDown => write!(f, "PG-DN"),
        }?;

        write!(f, ">")
    }
}
impl<'a> Parser<'a> for InvisibleKey {
    fn parse(input: &'a str) -> Result<(Self, &'a str), ParserError<'a>> {
        if !input.starts_with('<') {
            Err(ParserError::None)
        } else if let Some(end) = input.find('>') {
            match &input[1..end] {
                "F" => {
                    let fkey = input[2..].chars().try_fold(0_u8, |fold, ch| {
                        let err = || ParserError::UnknownSpecialKey(&input[1..end]);

                        fold.checked_mul(10)
                            .ok_or_else(err)?
                            .checked_add(ch.to_digit(10).ok_or_else(err)? as u8)
                            .ok_or_else(err)
                    })?;

                    Ok((Self::F(fkey), &input[end + 1..]))
                }
                "PG-UP" => Ok((Self::PageUp, &input[end + 1..])),
                "PG-DN" => Ok((Self::PageDown, &input[end + 1..])),
                key => Err(ParserError::UnknownSpecialKey(key)),
            }
        } else {
            Err(ParserError::UnclosedSpecialKey(input))
        }
    }
}

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
    /// Add a new key or append to the current tail if they share modifiers and are both textual
    pub fn push(&mut self, key: Key<'a>) {
        match (self.0.last_mut(), key) {
            (
                Some(Key {
                    kind: KeyKind::Visible(last_text),
                    mods: last_mods,
                }),
                Key {
                    kind: KeyKind::Visible(next_text),
                    mods: next_mods,
                },
            ) if next_mods.eq(last_mods) => last_text.to_mut().push_str(&next_text),
            (_, key) => self.0.push(key),
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
impl Display for KeyModifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Alt => write!(f, "M-"),
            Self::Control => write!(f, "C-"),
            Self::Shift => write!(f, "S-"),
            Self::Super => write!(f, "L-"),
        }
    }
}
impl<'a> Parser<'a> for KeyModifier {
    fn parse(input: &'a str) -> Result<(Self, &'a str), ParserError<'a>> {
        macro_rules! parse_modifiers {
            ($(($head:expr, $modifier:expr)),* $(,)?) => {
                match input {
                    $(input if input.starts_with($head) => Ok(($modifier, &input[const {
                        $head.len()
                    }..])),)*
                    _ => Err(ParserError::None),
                }
            }
        }

        parse_modifiers![
            ("M-", Self::Alt),
            ("C-", Self::Control),
            ("S-", Self::Shift),
            ("L-", Self::Super),
        ]
    }
}

#[derive(Debug, PartialEq)]
pub struct KeyModifiers(EnumMap<KeyModifier, bool>);
impl KeyModifiers {
    pub fn new() -> Self {
        Self::from_fn(|_| false)
    }

    pub fn from_fn<F>(f: F) -> Self
    where
        F: FnMut(KeyModifier) -> bool,
    {
        Self(EnumMap::from_fn(f))
    }

    /// Returns whether there are any active key modifiers
    pub fn is_active(&self) -> bool {
        self.0.values().copied().any(|active| active)
    }

    pub fn push(&mut self, modifier: KeyModifier) {
        self.0[modifier] = true;
    }
}
impl Display for KeyModifiers {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.0
            .iter()
            .filter_map(|(modifier, active)| active.then_some(modifier))
            .try_for_each(|modifier| write!(f, "{}", modifier))
    }
}
impl<'a> Parser<'a> for KeyModifiers {
    fn parse(mut input: &'a str) -> Result<(Self, &'a str), ParserError<'a>> {
        let mut output = Self::new();
        while let Ok((modifier, next_input)) = KeyModifier::parse(input) {
            output.push(modifier);
            input = next_input;
        }

        if output.is_active() {
            Ok((output, input))
        } else {
            Err(ParserError::None)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ParserError<'a> {
    None,
    UnknownSpecialKey(&'a str),
    UnclosedSpecialKey(&'a str),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_sequence_contains() {
        assert!(
            KeySequence::from_iter([
                Key {
                    kind: "foo".into(),
                    mods: KeyModifiers::from_fn(|_| false)
                },
                Key {
                    kind: "bar".into(),
                    mods: KeyModifiers::from_fn(|_| true)
                },
            ])
            .contains(&KeySequence::from_iter([Key {
                kind: "foo".into(),
                mods: KeyModifiers::from_fn(|_| false)
            }]))
        );
    }

    #[test]
    fn key_sequence_flatten() {
        assert_eq!(
            KeySequence::from_iter([
                Key {
                    kind: "foo".into(),
                    mods: KeyModifiers::from_fn(|_| false)
                },
                Key {
                    kind: "bar".into(),
                    mods: KeyModifiers::from_fn(|_| false)
                },
                Key {
                    kind: "foo".into(),
                    mods: KeyModifiers::from_fn(|_| true)
                },
                Key {
                    kind: "bar".into(),
                    mods: KeyModifiers::from_fn(|_| true)
                },
            ]),
            KeySequence::from_iter([
                Key {
                    kind: "foobar".into(),
                    mods: KeyModifiers::from_fn(|_| false)
                },
                Key {
                    kind: "foobar".into(),
                    mods: KeyModifiers::from_fn(|_| true)
                },
            ])
        );
    }
}
