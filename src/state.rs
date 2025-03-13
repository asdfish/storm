use {
    crate::CalloopData,
    smithay::{
        reexports::{
            calloop::EventLoop,
            wayland_server::{backend::ClientData, Display, DisplayHandle},
        },
        wayland::compositor::{CompositorClientState, CompositorHandler, CompositorState},
    },
};

mod handler;

#[derive(Debug)]
pub struct Storm {
    compositor_state: CompositorState,

    display_handle: DisplayHandle,
}
impl Storm {
    pub fn new(_: &mut EventLoop<CalloopData>, display: Display<Self>) -> Self {
        let display_handle = display.handle();

        Self {
            compositor_state: CompositorState::new::<Self>(&display_handle),

            display_handle,
        }
    }
}

#[derive(Default)]
struct ClientState {
    compositor_state: CompositorClientState,
}
impl ClientData for ClientState {}
