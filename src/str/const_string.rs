//! Owned string that can be modified in const contexts

use std::{
    fmt::{self, Debug, Display, Formatter},
    ops::{Deref, DerefMut},
    slice, str,
};

#[derive(Clone, Copy)]
/// Owned string that can be modified in const contexts
///
/// # Invariants
///
/// 1. Everything must be valid utf8.
/// 2. The length must never be over `N`.
/// 3. Length must be a valid char boundary.
pub struct ConstString<const N: usize> {
    buf: [u8; N],
    len: usize,
}
impl<const N: usize> Debug for ConstString<N> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.as_str())
    }
}
impl<const N: usize> Display for ConstString<N> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
impl<const N: usize> ConstString<N> {
    pub const fn as_bytes(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.buf.as_ptr(), self.len) }
    }
    /// # Safety
    ///
    /// Ensure all modifications are utf8
    pub const unsafe fn as_mut_bytes(&mut self) -> &mut [u8] {
        unsafe { slice::from_raw_parts_mut(self.buf.as_mut_ptr(), self.len) }
    }

    pub const fn as_str(&self) -> &str {
        // SAFETY: these should be safe if the invariants are all true
        unsafe { str::from_utf8_unchecked(self.as_bytes()) }
    }
    pub const fn as_mut_str(&mut self) -> &mut str {
        // SAFETY: these should be safe if the invariants are all true
        unsafe { str::from_utf8_unchecked_mut(self.as_mut_bytes()) }
    }

    pub const fn new() -> Self {
        Self {
            buf: [0; N],
            len: 0,
        }
    }

    pub const fn new_filled(with: u8) -> Self {
        Self {
            buf: [with; N],
            len: N,
        }
    }

    /// Append the given [char] to the end.
    ///
    /// # Panics
    ///
    /// Will panic if the [char] cannot be encoded.
    pub const fn push(&mut self, ch: char) {
        // `ch.encode_utf8().len()` returns a valid utf8 length
        self.len += ch
            .encode_utf8(unsafe {
                slice::from_raw_parts_mut(self.buf.as_mut_ptr().add(self.len), N - self.len)
            })
            .len();
    }

    /// # Panics
    ///
    /// Will panic if the [prim@str] is too long.
    pub const fn push_str(&mut self, str: &str) {
        if str.is_empty() {
            return;
        }

        assert!(self.len + str.len() <= N);

        // SAFETY: bounds checking is done above
        unsafe {
            str.as_ptr()
                .copy_to(self.buf.as_mut_ptr().add(self.len), str.len())
        }
        self.len += str.len();
    }
}
impl<const N: usize> PartialEq for ConstString<N> {
    fn eq(&self, rhs: &Self) -> bool {
        self.as_str() == rhs.as_str()
    }
}
impl<const N: usize> PartialEq<str> for ConstString<N> {
    fn eq(&self, rhs: &str) -> bool {
        self.as_str() == rhs
    }
}
impl<const N: usize> PartialEq<&str> for ConstString<N> {
    fn eq(&self, rhs: &&str) -> bool {
        self.as_str() == *rhs
    }
}
impl<const N: usize> Default for ConstString<N> {
    fn default() -> Self {
        const { Self::new() }
    }
}
impl<const N: usize> AsRef<str> for ConstString<N> {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
impl<const N: usize> AsMut<str> for ConstString<N> {
    fn as_mut(&mut self) -> &mut str {
        self.as_mut_str()
    }
}
impl<const N: usize> Deref for ConstString<N> {
    type Target = str;
    fn deref(&self) -> &str {
        self.as_str()
    }
}
impl<const N: usize> DerefMut for ConstString<N> {
    fn deref_mut(&mut self) -> &mut str {
        self.as_mut_str()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn const_string_new_filled() {
        let string = ConstString::<10>::new_filled(b' ');
        assert_eq!(string, "          ");
    }

    #[test]
    #[should_panic]
    fn const_string_push_fail() {
        let mut string = ConstString::<2>::new();
        string.push('f');
        string.push('o');
        string.push('o');
        assert_eq!(string, "foo");
    }
    #[test]
    fn const_string_push() {
        let mut string = ConstString::<3>::new();
        string.push('f');
        string.push('o');
        string.push('o');
        assert_eq!(string, "foo");
    }

    #[test]
    fn const_string_push_str() {
        let mut string = ConstString::<7>::new();
        string.push_str("good");
        string.push_str("bye");
        assert_eq!(string, "goodbye");
    }

    #[test]
    #[should_panic]
    fn const_string_push_str_fail() {
        let mut string = ConstString::<6>::new();
        string.push_str("good");
        string.push_str("bye");
        assert_eq!(string, "goodbye");
    }
}
