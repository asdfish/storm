use {
    crate::{
        backend::{self, Window},
        config::{Config, LogLevel},
    },
    std::{
        collections::{HashMap, HashSet},
        marker::PhantomData,
        num::NonZeroU8,
        sync::mpsc,
        thread,
    },
};

pub struct Storm<S, W, E>
where
    S: backend::State<W, E>,
    W: Window,
{
    backend_state: S,
    config: Config<'static>,
    rx: mpsc::Receiver<Event>,
    workspace: u8,
    workspaces: HashMap<u8, HashSet<W>>,

    _marker: PhantomData<E>,
}
impl<S, W, E> Storm<S, W, E>
where
    S: backend::State<W, E>,
    W: Window,
{
    pub fn new(config: Config<'static>) -> Result<Self, E> {
        let (tx, rx) = mpsc::channel();
        let mut workspaces = HashMap::new();

        Ok(Self {
            backend_state: S::new(&mut workspaces, tx)?,
            config,
            rx,
            // We start at one since most keyboards have 1 at the top left.
            workspace: 1,
            workspaces,

            _marker: PhantomData,
        })
    }
    pub fn run(mut self) -> Result<(), E> {
        loop {
            match self.rx.recv() {
                Ok(event) => match event {
                    Event::Key(_) => println!("key event"),
                }
                Err(error) => {
                    self.config.log(LogLevel::Verbose, |f| {
                        writeln!(f, "all senders have disconnected: {}", error)
                    });
                    break Ok(());
                }
            }
        }
    }
}

pub enum Event {
    Key(String),
}
