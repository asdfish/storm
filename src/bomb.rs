use std::{
    convert::{AsMut, AsRef},
    ops::{Deref, DerefMut},
};

pub struct Bomb<T, P>
where
    P: for<'a> FnMut(&'a mut T),
{
    data: Option<T>,
    payload: P,
}
impl<T, P> Bomb<T, P>
where
    P: for<'a> FnMut(&'a mut T),
{
    pub const fn new(data: T, payload: P) -> Self {
        Self {
            data: Some(data),
            payload: payload,
        }
    }

    /// Disable [Self::payload] and return [Self::data].
    pub fn diffuse(mut self) -> T {
        self.data.take().unwrap()
    }
}
impl<T, P> Drop for Bomb<T, P>
where
    P: for<'a> FnMut(&'a mut T),
{
    fn drop(&mut self) {
        if let Some(data) = &mut self.data {
            (self.payload)(data);
        }
    }
}
impl<T, P> AsRef<T> for Bomb<T, P>
where
    P: for<'a> FnMut(&'a mut T),
{
    fn as_ref(&self) -> &T {
        self.data.as_ref().unwrap()
    }
}
impl<T, P> AsMut<T> for Bomb<T, P>
where
    P: for<'a> FnMut(&'a mut T),
{
    fn as_mut(&mut self) -> &mut T {
        self.data.as_mut().unwrap()
    }
}
impl<T, P> Deref for Bomb<T, P>
where
    P: for<'a> FnMut(&'a mut T),
{
    type Target = T;
    fn deref(&self) -> &T {
        self.as_ref()
    }
}
impl<T, P> DerefMut for Bomb<T, P>
where
    P: for<'a> FnMut(&'a mut T),
{
    fn deref_mut(&mut self) -> &mut T {
        self.as_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diffuse() {
        let mut exploded = false;

        {
            let mut bomb = Bomb::new((), |_| exploded = true);
            bomb.diffuse();
        }

        assert_eq!(exploded, false);
    }
    #[test]
    fn explode() {
        let mut exploded = false;

        {
            let mut bomb = Bomb::new((), |_| exploded = true);
        }

        assert_eq!(exploded, true);
    }
}
