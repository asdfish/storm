#[cfg(windows)]
pub mod windows;

use std::io;

pub trait Window {
    type Error;
    type String;

    fn is_alive(&self) -> bool;
    fn is_focused(&self) -> bool;
    fn is_visible(&self) -> bool;

    fn move_to(&self, _: Rect) -> Result<(), Self::Error>;
    fn position(&self) -> Result<Rect, Self::Error>;

    fn title(&self) -> Result<Self::String, Self::Error>;

    fn set_focus(&mut self, _: bool) -> Result<(), Self::Error>;
    fn set_visibility(&mut self, _: bool) -> Result<(), Self::Error>;
}

pub struct Rect {
    x: i16,
    y: i16,
    width: u16,
    height: u16,
}
