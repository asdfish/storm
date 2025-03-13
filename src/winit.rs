use {
    crate::{
        CalloopData,
        attempt::{Attempt, DEFAULT_ATTEMPTS, StderrLogger},
        config::Verbosity,
        state::Storm,
    },
    smithay::{
        backend::{renderer::gles::GlesRenderer, winit},
        output::{Mode, Output, PhysicalProperties, Subpixel},
        reexports::calloop::EventLoop,
    },
    std::num::NonZeroUsize,
};

pub fn init(
    verbosity: Verbosity,
    event_loop: &mut EventLoop<CalloopData>,
    data: &mut CalloopData,
) -> Result<(), winit::Error> {
    let display_handle = &mut data.display_handle;
    let state = &mut data.state;

    let (mut backend, winit) = Attempt::new(
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
    output.create_global::<Storm>(display_handle);

    Ok(())
}
