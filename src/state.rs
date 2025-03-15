use {
    crate::{
        backend::{self, Window},
        config::{Config, LogLevel},
    },
    std::{
        collections::{HashMap, HashSet},
        marker::PhantomData,
        num::NonZeroU8,
        sync::mpsc::{Receiver, Sender, channel},
    },
};

pub struct Storm<'a, S, W, E>
where
    S: backend::State<W, E>,
    W: Window,
{
    backend_state: S,
    config: Config<'a>,
    event_receiver: Receiver<Event>,
    workspace: u8,
    windows: HashMap<u8, HashSet<W>>,

    _marker: PhantomData<E>,
}
impl<'a, S, W, E> Storm<'a, S, W, E>
where
    S: backend::State<W, E>,
    W: Window,
{
    pub fn new(config: Config<'a>) -> Result<Self, E> {
        let (sender, event_receiver) = channel();

        let mut windows = HashMap::new();
        let backend_state = S::new(&mut windows, sender)?;

        Ok(Self {
            backend_state,
            config,
            event_receiver,
            // We start at one since most keyboards have 1 at the top left.
            workspace: 1,
            windows,

            _marker: PhantomData,
        })
    }
    pub fn run(&mut self) {
        loop {
            match self.event_receiver.recv() {
                Ok(event) => {}
                Err(error) => {
                    self.config.log(LogLevel::Quiet, |f| {
                        writeln!(f, "all senders have disconnected: {}", error)
                    });
                    break;
                }
            }
        }
    }
}

pub enum Event {
    Key(String),
}
