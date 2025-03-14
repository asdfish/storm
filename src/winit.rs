use {
    crate::{
        CalloopData,
        attempt::{Attempt, DEFAULT_ATTEMPTS, StderrLogger},
        config::Verbosity,
        state::Storm,
    },
    smithay::{
        backend::{renderer::{
            damage::OutputDamageTracker,
            element::surface::WaylandSurfaceRenderElement,
            gles::GlesRenderer,
        }, winit::{self, WinitEvent}},
        desktop::space::render_output,
        output::{Mode, Output, PhysicalProperties, Subpixel},
        reexports::calloop::EventLoop,
        utils::{Rectangle, Transform},
    },
};

pub fn init(
    verbosity: Verbosity,
    event_loop: &mut EventLoop<CalloopData>,
    data: &mut CalloopData,
) -> Result<(), winit::Error> {
    let (backend, winit) = Attempt::new(
        DEFAULT_ATTEMPTS,
        StderrLogger::new("creating an instance of winit", verbosity),
        winit::init::<GlesRenderer>,
        |e: &winit::Error| !matches!(e, winit::Error::NotSupported),
    )
    .execute()?;

    let mode = Mode {
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
        },
    );
    let _global = output.create_global::<Storm>(&mut data.display_handle);
    output.change_current_state(Some(mode), Some(Transform::Flipped180), None, Some((0, 0).into()));
    output.set_preferred(mode);
    let mut damage_tracker = OutputDamageTracker::from_output(&output);

    event_loop
        .handle()
        .insert_source(
            winit,
            move |event, _, data| {
                match event {
                    WinitEvent::Resized { size, .. } => {
                        output.change_current_state(
                            Some(Mode {
                                size,
                                refresh: 60_000,
                            }),
                            None,
                            None,
                            None,
                        );
                    }
                    WinitEvent::Redraw => {
                        let size = backend.window_size();
                        let damage = Rectangle::from_size(size);

                        {
                            let (renderer, mut framebuffer) = backend.bind()
                                .unwrap();

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
                                [&data.state.space],
                                &[],
                                &mut damage_tracker,
                                [0.1, 0.1, 0.1, 1.0],
                            ).unwrap();
                        }
                    }
                    WinitEvent::CloseRequested => {
                        data.state.loop_signal.stop();
                    }
                    _ => {}
                }
            }
        );

    Ok(())
}
