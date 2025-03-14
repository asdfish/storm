#[cfg(windows)]
pub mod windows;

use std::io;

pub enum BackendError<E> {
    Io(io::Error),
    Specific(E),
}

pub trait Window<E> {
    fn is_alive(&self) -> bool;
    fn is_focused(&self) -> bool;
    fn is_visible(&self) -> bool;

    fn move_to(&self, _: Rect) -> Result<(), BackendError<E>>;
    fn position(&self) -> Result<Rect, BackendError<E>>;

    fn name(&self) -> Result<String, BackendError<E>>;
    fn title(&self) -> Result<String, BackendError<E>>;

    fn set_focus(&mut self, _: bool) -> Result<(), BackendError<E>>;
    fn set_visibility(&mut self, _: bool) -> Result<(), BackendError<E>>;
}

pub struct Rect {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}
