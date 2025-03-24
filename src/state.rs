use {
    crate::{
        backend::{self, Window},
        config::{Config, key::{Key, KeySequence}},
        error,
    },
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
{    pub backend_state: S,
    config: Config<'a>,
    rx: EventReceiver<W, E>,
    pub workspace: u8,
    pub workspaces: HashMap<u8, Vec<W>>,

    max_key_binding_len: usize,
    pressed_keys: KeySequence<'a>,

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

        let max_key_binding_len = config.max_key_binding_len();

        Ok(Self {
            backend_state: S::new(&mut workspaces, tx)?,
            config,
            rx,
            // We start at one since most keyboards have 1 at the top left.
            workspace: 1,
            workspaces,

            max_key_binding_len,
            pressed_keys: KeySequence::with_capacity(max_key_binding_len),

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
                    Ok(Event::Key(consume, key)) => {
                        let _ = consume.send(false);
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
    Key(oneshot::Sender<bool>, Key<'static>),
}
