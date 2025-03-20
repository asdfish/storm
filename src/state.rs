use {
    crate::{
        backend::{self, Window},
        config::{Config, key::KeyModifiers},
        error,
    },
    enum_map::{Enum, EnumMap},
    oneshot,
    std::{
        collections::{HashMap, hash_map},
        fmt::Display,
        marker::PhantomData,
        sync::mpsc,
    },
};

pub type EventSender<W, E> = mpsc::Sender<Result<Event<W>, E>>;
pub type EventReceiver<W, E> = mpsc::Receiver<Result<Event<W>, E>>;

pub struct Storm<'a, S, W, E>
where
    E: Display,
    S: backend::State<W, E>,
    W: Window,
{
    pub backend_state: S,
    config: Config<'a>,
    rx: EventReceiver<W, E>,
    pub workspace: u8,
    pub workspaces: HashMap<u8, Vec<W>>,

    _marker: PhantomData<E>,
}
impl<'a, S, W, E> Storm<'a, S, W, E>
where
    E: Display,
    S: backend::State<W, E>,
    W: Window,
{
    fn tile_windows(&self) {}

    pub fn new(config: Config<'a>) -> Result<Self, E> {
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
                    Ok(Event::AddWindow { workspace, window }) => {
                        match self.workspaces.entry(workspace) {
                            hash_map::Entry::Occupied(mut entry) => {
                                entry.get_mut().push(window);
                            }
                            hash_map::Entry::Vacant(entry) => {
                                entry.insert(Vec::from([window]));
                            }
                        }

                        if workspace == self.workspace {
                            self.tile_windows();
                        }
                    }
                    Ok(Event::Key(consume, modifiers, key)) => {
                        println!("key event: {:?}", key);
                        println!("modifiers, {:?}", modifiers);
                        consume.send(false).expect(error::CLOSED_CHANNEL);
                    }
                    Err(e) => self
                        .config
                        .error(|f| writeln!(f, "failed to process event: {}", e)),
                },
                Err(error) => {
                    self.config
                        .error(|f| writeln!(f, "all senders have disconnected: {}", error));
                    break Ok(());
                }
            }
            S::each_event(&mut self);
        }
    }
}

/// Events to be received in [Storm::run].
pub enum Event<W: Window> {
    /// Add a window.
    AddWindow {
        workspace: u8,
        window: W,
    },
    Key(oneshot::Sender<bool>, KeyModifiers, String),
}
