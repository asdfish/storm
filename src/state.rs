mod handler;

use {
    smithay::{
        backend::{
            input::{InputEvent, KeyboardKeyEvent},
            winit::{self, WinitEvent},
            renderer::{
                damage::OutputDamageTracker,
                element::surface::WaylandSurfaceRenderElement,
                gles::GlesRenderer,
            },
        },
        desktop::{
            space::{
                Space,
                render_output,
            },
            Window,
        },
        input::{keyboard::FilterResult, Seat, SeatState},
        output::{
            Output,
            Mode as OutputMode,
            PhysicalProperties,
            Subpixel,
        },
        reexports::{
            calloop::{
                EventLoop,
                generic::Generic,
                Interest,
                LoopHandle,
                LoopSignal,
                Mode as CalloopMode,
                PostAction,
            },
            wayland_server::{
                Client,
                Display, DisplayHandle, backend::ClientData
            },
        },
        utils::Rectangle,
        wayland::{
            compositor::{CompositorClientState, CompositorState},
            selection::data_device::DataDeviceState,
            shell::xdg::XdgShellState,
            shm::ShmState,
            socket::ListeningSocketSource,
        },
    },
    std::{
        collections::HashSet,
        env, sync::Arc, time::{Duration, Instant},
    },
};

#[derive(Debug)]
pub struct Storm {
    compositor_state: CompositorState,
    data_device_state: DataDeviceState,
    seat_state: SeatState<Self>,
    shm_state: ShmState,
    xdg_shell_state: XdgShellState,

    clients: HashSet::<Client>,
    display_handle: DisplayHandle,
    loop_signal: LoopSignal,
    seat: Seat<Self>,
    space: Space<Window>,
    start_time: Instant,
}
impl Storm {
    pub fn new(event_loop: &EventLoop<Self>) -> Self {
        let loop_handle = event_loop.handle();

        let display = Display::<Self>::new().unwrap();
        let display_handle = display.handle();

        let mut seat_state = SeatState::new();
        let mut seat = seat_state.new_seat("winit");
        seat.add_pointer();
        let keyboard = seat.add_keyboard(Default::default(), 200, 25).unwrap();

        let (mut backend, winit) = winit::init::<GlesRenderer>().unwrap();
        let mode = OutputMode {
            size: backend.window_size(),
            refresh: 60_000,
        };
        let output = Output::new(
            String::from("winit"),
            PhysicalProperties {
                size: (0, 0).into(),
                subpixel: Subpixel::Unknown,
                make: String::from("Smithay"),
                model: String::from("Winit"),
            }
        );
        output.change_current_state(Some(mode), None, None, None);
        output.set_preferred(mode);
        let mut damage_tracker = OutputDamageTracker::from_output(&output);

        let socket = ListeningSocketSource::new_auto().unwrap();
        unsafe {
            env::set_var("WAYLAND_DISPLAY", socket.socket_name());
        }
        loop_handle.insert_source(socket, |client_stream, _, state| {
            state.display_handle
                .insert_client(client_stream, Arc::new(ClientState::default()))
                .unwrap();
        }).unwrap();
        loop_handle.insert_source(
            Generic::new(display, Interest::READ, CalloopMode::Level),
            |_, display, state| {
                unsafe {
                    display.get_mut()
                        .dispatch_clients(state)
                        .unwrap();
                }

                Ok(PostAction::Continue)
            }
        ).unwrap();

        loop_handle.insert_source(
            winit,
            move |event, _, state| {
                match event {
                    WinitEvent::Resized { size, .. } => {
                        output.change_current_state(
                            Some(OutputMode {
                                size,
                                refresh: 60_000,
                            }),
                            None,
                            None,
                            None,
                        );
                        backend.window().request_redraw();
                    }
                    WinitEvent::Redraw => {
                        let size = backend.window_size();
                        let damage = Rectangle::from_size(size);

                        backend.bind().unwrap();
                        let renderer = backend.renderer();
                        render_output::<
                            _,
                            WaylandSurfaceRenderElement<GlesRenderer>,
                            _,
                            _,
                            >(
                            &output,
                            renderer,
                            1.0,
                            0,
                            [&state.space],
                            &[],
                            &mut damage_tracker,
                            [0.1, 0.1, 0.1, 1.0]
                        ).unwrap();
                        backend.submit(Some(&[damage])).unwrap();

                        state.space.elements()
                            .for_each(|window| {
                                window.send_frame(
                                    &output,
                                    state.start_time.elapsed(),
                                    Some(Duration::ZERO),
                                    |_, _| Some(output.clone()),
                                )
                            });
                        state.space.refresh();
                    }
                    WinitEvent::Input(event) => match event {
                        InputEvent::Keyboard { event } => {
                            keyboard.input::<(), _>(
                                state,
                                event.key_code(),
                                event.state(),
                                0.into(),
                                0,
                                |_, _, _| {
                                    FilterResult::Forward
                                }
                            );
                        }
                        InputEvent::PointerMotionAbsolute { .. } => {
                            if let Some(surface) = state.xdg_shell_state.toplevel_surfaces().first() {
                                let surface = surface.wl_surface().clone();
                                keyboard.set_focus(state, Some(surface), 0.into());
                            }
                        }
                        _ => {}
                    }
                    WinitEvent::CloseRequested => state.loop_signal.stop(),
                    _ => {},
                }
            }
        ).unwrap();

        Self {
            compositor_state: CompositorState::new::<Self>(&display_handle),
            data_device_state: DataDeviceState::new::<Self>(&display_handle),
            seat_state,
            shm_state: ShmState::new::<Self>(&display_handle, []),
            xdg_shell_state: XdgShellState::new::<Self>(&display_handle),

            clients: HashSet::new(),
            display_handle,
            loop_signal: event_loop.get_signal(),
            seat,
            space: Space::default(),
            start_time: Instant::now(),
        }
    }
}

#[derive(Default)]
pub struct ClientState {
    compositor_client_state: CompositorClientState,
}
impl ClientData for ClientState {}
