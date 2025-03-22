use {
    enum_map::{Enum, EnumMap},
    smallvec::SmallVec,
    std::{
        borrow::Cow,
        fmt::{self, Display, Formatter},
        str,
        ops::Not,
    },
};

pub trait Parser<'a>: Sized {
    fn parse(_: &'a str) -> Option<Result<(Self, &'a str), ParserError<'a>>>;
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ParserError<'a> {
    UnusedEscape { src: &'a str, index: usize },
    UnknownSpecialKey(&'a str),
    UnclosedSpecialKey(&'a str),
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
impl<'a> Key<'a> {
    pub const fn new(mods: KeyModifiers, kind: KeyKind<'a>) -> Self {
        Self {
            mods,
            kind,
        }
    }
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
impl<'a> Parser<'a> for Key<'a> {
    fn parse(input: &'a str) -> Option<Result<(Key<'a>, &'a str), ParserError<'a>>> {
        let (modifiers, input) = match KeyModifiers::parse(input).transpose() {
            Ok(o) => o,
            Err(err) => return Some(Err(err)),
        }
        .unwrap_or_else(|| (KeyModifiers::default(), input));
        let (kind, input) = match KeyKind::parse(input)? {
            Ok(o) => o,
            Err(err) => return Some(Err(err)),
        };

        Some(Ok((Key::new(modifiers, kind), input)))
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
                'M' | 'C' | 'S' | 'L' | '<' => write!(f, "\\{}", ch),
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
    fn parse(input: &'a str) -> Option<Result<(Self, &'a str), ParserError<'a>>> {
        match input {
            "" => None,
            input if input.starts_with('<') => InvisibleKey::parse(input)
                .map(|result| result.map(|(key, next)| (KeyKind::Invisible(key), next))),
            input => {
                let mut keys = Cow::Borrowed("");
                let mut chars = input.char_indices().peekable();

                while let Some((index, ch)) =
                    chars.next_if(|(_, ch)| !matches!(ch, 'M' | 'C' | 'S' | 'L' | '<'))
                {
                    match ch {
                        '\\' => keys.to_mut().push(
                            match chars
                                .next()
                                .ok_or(ParserError::UnusedEscape { src: input, index })
                            {
                                Ok((_, ch)) => match ch {
                                    'n' => '\n',
                                    'r' => '\r',
                                    't' => '\t',
                                    ch => ch,
                                },
                                Err(err) => return Some(Err(err)),
                            },
                        ),
                        ch => match &mut keys {
                            Cow::Borrowed(keys) => *keys = &input[..=index],
                            Cow::Owned(keys) => keys.push(ch),
                        },
                    }
                }
                if keys.is_empty() {
                    return None;
                }

                Some(Ok((
                    KeyKind::Visible(keys),
                    chars
                        .next()
                        .and_then(|(i, _)| input.get(i..))
                        .unwrap_or(&input[input.len()..]),
                )))
            }
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
            Self::F(n) => write!(f, "F-{n}"),
            Self::PageUp => write!(f, "PG-UP"),
            Self::PageDown => write!(f, "PG-DN"),
        }?;

        write!(f, ">")
    }
}
impl<'a> Parser<'a> for InvisibleKey {
    fn parse(input: &'a str) -> Option<Result<(Self, &'a str), ParserError<'a>>> {
        if input.is_empty() || !input.starts_with('<') {
            None
        } else if let Some(end) = input.find('>') {
            let next = &input[end..];

            match &input[1..end] {
                "PG-UP" => Some(Ok((InvisibleKey::PageUp, next))),
                "PG-DN" => Some(Ok((InvisibleKey::PageDown, next))),
                fkey if fkey.starts_with("F-") => fkey
                    .chars()
                    .skip(2)
                    .try_fold(0_u8, |fold, next| {
                        let err = || ParserError::UnknownSpecialKey(fkey);

                        fold.checked_mul(10)
                            .ok_or_else(err)?
                            .checked_add(next.to_digit(10).ok_or_else(err)? as u8)
                            .ok_or_else(err)
                    })
                    .map(|i| (InvisibleKey::F(i), next))
                    .map(Some)
                    .transpose(),
                unknown => Some(Err(ParserError::UnknownSpecialKey(unknown))),
            }
        } else {
            Some(Err(ParserError::UnclosedSpecialKey(input)))
        }
    }
}

#[derive(Debug, Default, PartialEq)]
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
impl Display for KeySequence<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|key| write!(f, "{}", key))
    }
}
impl<'a> Parser<'a> for KeySequence<'a> {
    fn parse(mut input: &'a str) -> Option<Result<(KeySequence<'a>, &'a str), ParserError<'a>>> {
        let mut some = false;
        let mut key_seq = KeySequence::new();

        while let Some((key, next_input)) = match Key::parse(input).transpose() {
            Ok(o) => o,
            Err(err) => return Some(Err(err)),
        } {
            some = true;
            key_seq.push(key);
            input = next_input;
        }

        some.then_some(Ok((key_seq, input)))
    }
}

#[derive(Clone, Copy, Debug, Enum, PartialEq)]
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
impl KeyModifier {
    pub const VARIANTS: [(&str, Self); 4] = [
        ("M-", Self::Alt),
        ("C-", Self::Control),
        ("S-", Self::Shift),
        ("L-", Self::Super),
    ];
}
impl Display for KeyModifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", Self::VARIANTS[*self as usize].0)
    }
}
impl<'a> Parser<'a> for KeyModifier {
    fn parse(input: &'a str) -> Option<Result<(KeyModifier, &'a str), ParserError<'a>>> {
        match input {
            input if input.starts_with("M-") => Some(Ok((KeyModifier::Alt, &input[2..]))),
            input if input.starts_with("C-") => Some(Ok((KeyModifier::Control, &input[2..]))),
            input if input.starts_with("S-") => Some(Ok((KeyModifier::Shift, &input[2..]))),
            input if input.starts_with("L-") => Some(Ok((KeyModifier::Super, &input[2..]))),
            _ => None,
        }
    }
}

#[derive(Debug, Default, PartialEq)]
pub struct KeyModifiers(EnumMap<KeyModifier, bool>);
impl KeyModifiers {
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
impl Extend<KeyModifier> for KeyModifiers {
    fn extend<I>(&mut self, iter: I)
    where I: IntoIterator<Item = KeyModifier> {
        iter.into_iter()
            .for_each(|key_mod| self.push(key_mod))
    }
}
impl FromIterator<(KeyModifier, bool)> for KeyModifiers {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = (KeyModifier, bool)>,
    {
        Self::from_iter(
            iter.into_iter()
                .filter_map(|(key, active)| active.then_some(key)),
        )
    }
}
impl FromIterator<KeyModifier> for KeyModifiers {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = KeyModifier>,
    {
        let mut output = Self::default();
        output.extend(iter);

        output
    }
}
impl<'a> Parser<'a> for KeyModifiers {
    fn parse(mut input: &'a str) -> Option<Result<(KeyModifiers, &'a str), ParserError<'a>>> {
        let mut some = false;
        let mut key_mods = KeyModifiers::default();

        while let Some((key_mod, next_input)) = KeyModifier::parse(input).transpose().expect("internal error: the implementation of [KeyModifierParser::parse] should never return any errors")
        {
            some = true;
            key_mods.push(key_mod);

            input = next_input;
        }

        some.then_some(Ok((key_mods, input)))
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        itertools::Itertools,
        std::fmt::{Debug, Display, Write},
    };

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

    /// [test_parser_eq] with format checking.
    fn test_parser<I, S, T>(iter: I)
    where
        I: IntoIterator<Item = (S, T)>,
        S: AsRef<str>,
        T: Debug + Display + for<'a> Parser<'a> + PartialEq,
    {
        let mut input_buffer = String::new();
        let mut expected_buffer = String::new();

        iter.into_iter().for_each(|(input, expected)| {
            let input = T::parse(input.as_ref()).unwrap().unwrap().0;
            assert_eq!(input, expected);

            [
                (&mut input_buffer, &input),
                (&mut expected_buffer, &expected),
            ]
            .into_iter()
            .for_each(|(buffer, token)| {
                buffer.clear();
                write!(buffer, "{}", token).unwrap();
            });
            assert_eq!(input_buffer, expected_buffer);

            let display = T::parse(&expected_buffer).unwrap().unwrap().0;
            assert_eq!(display, expected);
        })
    }

    #[test]
    fn invisible_key() {
        test_parser((u8::MIN..=u8::MAX).map(|i| (format!("<F-{}>", i), InvisibleKey::F(i))));
        test_parser([
            ("<PG-UP>", InvisibleKey::PageUp),
            ("<PG-DN>", InvisibleKey::PageDown),
        ]);
    }

    #[test]
    fn key_modifier() {
        test_parser(KeyModifier::VARIANTS);
    }

    #[test]
    fn key_modifiers() {
        test_parser(
            (1..=4)
                .flat_map(|n| KeyModifier::VARIANTS.into_iter().permutations(n))
                .map(|modifiers| modifiers.into_iter().unzip::<_, _, String, KeyModifiers>()),
        );
    }

    #[test]
    fn key_kind() {
        assert_eq!(
            KeyKind::parse("foo bar<F-10>"),
            Some(Ok((KeyKind::Visible("foo bar".into()), "<F-10>"))),
        );

        const EXPECTED: KeyKind = KeyKind::Visible(Cow::Borrowed("<C"));

        let escaped_key = KeyKind::parse("\\<\\C").unwrap().unwrap().0;
        assert_eq!(escaped_key, EXPECTED);

        let display_key = format!("{}", escaped_key);
        let display_key = KeyKind::parse(&display_key).unwrap().unwrap().0;
        assert_eq!(display_key, EXPECTED);
    }

    // `Key*` types are just the above combined, so they won't have anything worth testing.
}
