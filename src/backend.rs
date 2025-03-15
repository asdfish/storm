#[cfg(windows)]
pub mod windows;

use {
    crate::state::{Event, Storm},
    std::{
        collections::{HashMap, HashSet},
        io,
        sync::mpsc::Sender,
    },
};

pub trait State<W, E>: Sized
where
    W: Window,
{
    /// Operate on windows before they get put into [Storm].
    fn new(_: &mut HashMap<u8, HashSet<W>>, _: Sender<Event>) -> Result<Self, E>;
    /// This function gets called whenever [Storm::receiver] receives an event. Useful for things
    /// that need to occur every event.
    fn on_receive(_: &mut Storm<Self, W, E>) {}
}

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
