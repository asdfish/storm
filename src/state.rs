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
    config: Config<'static>,
    workspace: u8,
    workspaces: HashMap<u8, HashSet<W>>,

    _marker: PhantomData<(S, E)>,
}
impl<S, W, E> Storm<S, W, E>
where
    S: backend::State<W, E>,
    W: Window,
{
    pub fn new(config: Config<'static>) -> Self {
        Self {
            config,
            // We start at one since most keyboards have 1 at the top left.
            workspace: 1,
            workspaces: HashMap::new(),

            _marker: PhantomData,
        }
    }
    pub fn run(mut self) -> Result<(), E> {
        let (event_sender, event_receiver) = mpsc::channel();
        let backend_state = S::new(&mut self.workspaces, event_sender)?;

        thread::spawn(move || {
            loop {
                match event_receiver.recv() {
                    Ok(event) => match event {
                        Event::Key(_) => {
                            println!("key event");
                        }
                    },
                    Err(error) => {
                        self.config.log(LogLevel::Verbose, |f| {
                            writeln!(f, "all senders have disconnected: {}", error)
                        });
                        return;
                    }
                }
            }
        });

        backend_state.run();
        Ok(())
    }
}

pub enum Event {
    Key(String),
}
