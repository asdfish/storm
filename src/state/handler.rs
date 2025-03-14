use {
    super::{ClientState, Storm},
    smithay::{
        backend::renderer::utils::on_commit_buffer_handler,
        desktop::Window,
        delegate_compositor, delegate_data_device, delegate_shm, delegate_xdg_shell,
        input::{SeatHandler, SeatState},
        reexports::{
            wayland_protocols::xdg::shell::server::xdg_toplevel::State,
            wayland_server::{
                Client,
                protocol::{wl_buffer::WlBuffer, wl_seat::WlSeat, wl_surface::WlSurface},
            },
        },
        utils::Serial,
        wayland::{
            buffer::BufferHandler,
            compositor::{CompositorClientState, CompositorHandler, CompositorState},
            selection::{
                SelectionHandler,
                data_device::{
                    ClientDndGrabHandler, DataDeviceHandler, DataDeviceState, ServerDndGrabHandler,
                },
            },
            shell::xdg::{
                PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
            },
            shm::{ShmHandler, ShmState},
        },
    },
};

impl BufferHandler for Storm {
    fn buffer_destroyed(&mut self, _buffer: &WlBuffer) {}
}
impl ClientDndGrabHandler for Storm {}
impl CompositorHandler for Storm {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }
    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        &client
            .get_data::<ClientState>()
            .unwrap()
            .compositor_client_state
    }
    fn commit(&mut self, surface: &WlSurface) {
        on_commit_buffer_handler::<Self>(surface)
    }
}
impl DataDeviceHandler for Storm {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.data_device_state
    }
}
impl SeatHandler for Storm {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<Self> {
        &mut self.seat_state
    }
}
impl SelectionHandler for Storm {
    type SelectionUserData = ();
}
impl ServerDndGrabHandler for Storm {}
impl ShmHandler for Storm {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}
impl XdgShellHandler for Storm {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        let window = Window::new_wayland_window(surface);
        self.space.map_element(window, (0, 0), false);
    }

    fn new_popup(&mut self, _surface: PopupSurface, _positioner: PositionerState) {
        // Handle popup creation here
    }

    fn grab(&mut self, _surface: PopupSurface, _seat: WlSeat, _serial: Serial) {
        // Handle popup grab here
    }

    fn reposition_request(
        &mut self,
        _surface: PopupSurface,
        _positioner: PositionerState,
        _token: u32,
    ) {
        // Handle popup reposition here
    }
}

delegate_compositor!(Storm);
delegate_data_device!(Storm);
delegate_shm!(Storm);
delegate_xdg_shell!(Storm);
