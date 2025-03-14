use {
    crate::CalloopData,
    smithay::{
        desktop::{
            space::Space,
            Window,
        },
        reexports::{
            calloop::{PostAction, EventLoop, generic::Generic, Interest, LoopSignal, Mode},
            wayland_server::{backend::ClientData, Display, DisplayHandle},
        },
        wayland::{
            compositor::{CompositorClientState, CompositorHandler, CompositorState},
            socket::ListeningSocketSource,
        },
    },
    std::{
        ffi::OsString,
        sync::Arc,
    },
};

mod handler;

#[derive(Debug)]
pub struct Storm {
    compositor_state: CompositorState,

    display_handle: DisplayHandle,
    pub loop_signal: LoopSignal,
    pub space: Space<Window>,
    socket_name: OsString,
}
impl Storm {
    pub fn new(event_loop: &mut EventLoop<CalloopData>, display: Display<Self>) -> Self {
        let display_handle = display.handle();

        Self {
            compositor_state: CompositorState::new::<Self>(&display_handle),

            display_handle,
            loop_signal: event_loop.get_signal(),
            space: Space::default(),
            socket_name: Self::init_wayland_listener(display, event_loop),
        }
    }

    fn init_wayland_listener(
        display: Display<Storm>,
        event_loop: &mut EventLoop<CalloopData>
    ) -> OsString {
        let listening_socket = ListeningSocketSource::new_auto()
            // TODO: error handling
            .unwrap();

        let socket_name = listening_socket.socket_name().to_os_string();

        let loop_handle = event_loop.handle();

        loop_handle
            .insert_source(listening_socket, move |client_stream, _, state| {
                state
                    .display_handle
                    .insert_client(client_stream, Arc::new(ClientState::default()))
                    // TODO: error handling
                    .unwrap();
            })
            // TODO: error handling
            .unwrap();

        loop_handle
            .insert_source(
                Generic::new(display, Interest::READ, Mode::Level),
                |_, display, state| {
                    // SAFETY: we don't drop display
                    unsafe {
                        display.get_mut()
                            .dispatch_clients(&mut state.state)
                            .unwrap();
                    }

                    Ok(PostAction::Continue)
                }
            )
            // TODO: error handling
            .unwrap();

        socket_name
    }
}

#[derive(Default)]
struct ClientState {
    compositor_state: CompositorClientState,
}
impl ClientData for ClientState {}
