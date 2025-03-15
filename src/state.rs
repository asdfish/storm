use {
    crate::{
        backend::{self, Window},
        config::{Config, LogLevel},
    },
    std::{
        collections::{HashMap, HashSet},
        fmt::Display,
        marker::PhantomData,
        sync::mpsc,
    },
};

pub struct Storm<S, W, E>
where
    E: Display,
    S: backend::State<W, E>,
    W: Window,
{
    backend_state: S,
    config: Config<'static>,
    rx: mpsc::Receiver<Result<Event, E>>,
    workspace: u8,
    workspaces: HashMap<u8, HashSet<W>>,

    _marker: PhantomData<E>,
}
impl<S, W, E> Storm<S, W, E>
where
    E: Display,
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
    pub fn run(self) -> Result<(), E> {
        loop {
            match self.rx.recv() {
                Ok(event) => match event {
                    Ok(Event::Key(key)) => println!("key event: {:?}", key),
                    Err(e) => eprintln!("failed to process event: {}", e),
                },
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
