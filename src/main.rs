#![no_main]

mod attempt;
mod config;
mod opts;
mod state;

use {
    attempt::{Always, Attempt, DEFAULT_ATTEMPTS},
    config::Config,
    state::Storm,
    smithay::reexports::calloop::EventLoop,
    std::{
        ffi::{c_char, c_int},
        time::Duration,
    },
};

// SAFETY: every c program has done this since the dawn of time
#[unsafe(no_mangle)]
fn main(argc: c_int, argv: *const *const c_char) -> c_int {
    let mut config = Config::default();
    // SAFETY: argc and argv should not be unsafe to dereference
    unsafe { config.apply(argc, argv) };

    let mut event_loop = EventLoop::<Storm>::try_new().unwrap();
    let mut storm = Storm::new(&event_loop);

    config.execute_commands();

    event_loop.run(
        None,
        //Some(Duration::from_millis(10)),
        &mut storm,
        |_data| {
            println!("something happened");
        }
    ).unwrap();

    0
}
