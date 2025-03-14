#![no_std]

use core::{
    convert::{AsMut, AsRef},
    mem::take,
    ops::{Deref, DerefMut},
};

pub struct Guard<T, F>
where
    F: FnOnce(&mut T),
{
    data: T,
    guard: Option<F>,
}
impl<T, F> Guard<T, F>
where
    F: FnOnce(&mut T),
{
    pub const fn new(data: T, guard: F) -> Self {
        Self {
            data,
            guard: Some(guard),
        }
    }
}
impl<T, F> AsRef<T> for Guard<T, F>
where
    F: FnOnce(&mut T),
{
    fn as_ref(&self) -> &T {
        &self.data
    }
}
impl<T, F> AsMut<T> for Guard<T, F>
where
    F: FnOnce(&mut T),
{
    fn as_mut(&mut self) -> &mut T {
        &mut self.data
    }
}
impl<T, F> Deref for Guard<T, F>
where
    F: FnOnce(&mut T),
{
    type Target = T;
    fn deref(&self) -> &T {
        self.as_ref()
    }
}
impl<T, F> DerefMut for Guard<T, F>
where
    F: FnOnce(&mut T),
{
    fn deref_mut(&mut self) -> &mut T {
        self.as_mut()
    }
}
impl<T, F> Drop for Guard<T, F>
where
    F: FnOnce(&mut T),
{
    fn drop(&mut self) {
        if let Some(guard) = self.guard.take() {
            guard(&mut self.data);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guard() {
        let mut dropped = false;

        {
            let guard = Guard::new((), |_| dropped = true);
        }

        assert!(dropped);
    }
}
