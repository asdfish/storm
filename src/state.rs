use {
    crate::{
        backend::{self, Window},
        config::{
            key::{Key, KeySequence},
            Config,
        },
    },
    std::{
        collections::{hash_map, HashMap},
        cmp::Ordering,
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

    max_key_binding_len: usize,
    pressed_keys: KeySequence<'a>,

    pub quit: bool,

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

            quit: false,

            _marker: PhantomData,
        })
    }

    pub fn run(mut self) -> Result<(), E> {
        while !self.quit {
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
                        // a response should be sent asap to allow the thread to continue

                        self.pressed_keys.push(key);
                        if self.pressed_keys.len() > self.max_key_binding_len {
                            let _ = consume.send(KeyIntercept::Allow);
                            self.pressed_keys.clear();
                            continue;
                        }

                        let mut lesser = false;
                        if let Some(key_action) =
                            self.config
                                .key_bindings
                                .iter()
                                .flat_map(|(action, sequences)| {
                                    sequences.iter().map(move |sequence| (action, sequence))
                                })
                                .map(|(action, sequence)| (action, self.pressed_keys.partial_cmp(sequence)))
                                .inspect(|(_, ord)| if *ord == Some(Ordering::Less) {
                                    lesser = true;
                                })
                                .find(|(_, ord)| *ord == Some(Ordering::Equal))
                                .map(|(action, _)| action) {
                                    let _ = consume.send(KeyIntercept::Allow);
                                    self.pressed_keys.clear();

                                    key_action.execute(&mut self);
                                } else if lesser {
                                    let _ = consume.send(KeyIntercept::Block);
                                } else {
                                    let _ = consume.send(KeyIntercept::Allow);
                                    self.pressed_keys.clear();
                                }
                    }
                    Err(e) => self
                        .config
                        .error(|f| writeln!(f, "failed to process event: {}", e)),
                },
                Err(error) => {
                    self.config
                        .error(|f| writeln!(f, "all senders have disconnected: {}", error));
                    break;
                }
            }
            S::each_event(&mut self);
        }

        Ok(())
    }
}

/// Events to be received in [Storm::run], sent from the platform specific backend.
pub enum Event<W: Window> {
    /// Add a window.
    AddWindow {
        workspace: u8,
        window: W,
    },
    Key(oneshot::Sender<KeyIntercept>, Key<'static>),
}
#[derive(Clone, Copy, Debug, Default)]
pub enum KeyIntercept {
    #[default]
    Allow,
    Block,
}
