use {
    crate::CalloopData,
    smithay::{
        delegate_compositor, delegate_output,
        reexports::{
            calloop::EventLoop,
            wayland_server::{Client, Display, DisplayHandle, protocol::wl_surface::WlSurface},
        },
        wayland::{
            compositor::{CompositorClientState, CompositorHandler, CompositorState},
            output::OutputHandler,
        },
    },
    super::{
        ClientState,
        Storm,
    },
};

impl CompositorHandler for Storm {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        &client.get_data::<ClientState>().unwrap().compositor_state
    }

    fn commit(&mut self, surface: &WlSurface) {
        todo!()
    }
}
impl OutputHandler for Storm {}

delegate_compositor!(Storm);
delegate_output!(Storm);
