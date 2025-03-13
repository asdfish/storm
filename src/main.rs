#![no_main]

mod attempt;
mod config;
mod opts;
mod state;
mod winit;

use {
    attempt::{Always, Attempt, DEFAULT_ATTEMPTS, StderrLogger},
    config::Config,
    smithay::reexports::{
        calloop::EventLoop,
        wayland_server::{Display, DisplayHandle, backend::InitError},
    },
    state::Storm,
    std::{
        ffi::{c_char, c_int},
        num::NonZeroUsize,
    },
};

#[derive(Debug)]
struct CalloopData {
    state: Storm,
    display_handle: DisplayHandle,
}

// SAFETY: every c program has done this since the dawn of time
#[unsafe(no_mangle)]
fn main(argc: c_int, argv: *const *const c_char) -> c_int {
    let mut config = Config::default();
    // SAFETY: argc and argv should not be unsafe to dereference
    unsafe { config.apply(argc, argv) };

    let mut event_loop = match Attempt::new(
        DEFAULT_ATTEMPTS,
        StderrLogger::new("creating an event loop", config.verbosity),
        || EventLoop::<CalloopData>::try_new(),
        Always,
    )
    .execute()
    {
        Ok(el) => el,
        Err(_) => unreachable!(),
    };

    let display = match Attempt::new(
        DEFAULT_ATTEMPTS,
        StderrLogger::new("creating a wayland display", config.verbosity),
        Display::<Storm>::new,
        |err: &InitError| !matches!(err, InitError::NoWaylandLib),
    )
    .execute()
    {
        Ok(display) => display,
        Err(err) => {
            config
                .verbosity
                .error(|| eprintln!("failed to create an event loop: {}", err));
            return 1;
        }
    };
    let display_handle = display.handle();

    let mut data = CalloopData {
        state: Storm::new(&mut event_loop, display),
        display_handle,
    };
    winit::init(config.verbosity, &mut event_loop, &mut data);

    0
}
