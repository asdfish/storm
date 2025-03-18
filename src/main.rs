#![cfg_attr(not(test), no_main)]

pub mod backend;
pub mod config;
pub mod error;
pub mod state;

use {
    config::Config,
    std::{
        env,
        ffi::{c_char, c_int},
    },
};

// SAFETY: every c program has done this since the dawn of time
#[cfg_attr(not(test), unsafe(no_mangle))]
fn main(argc: c_int, argv: *const *const c_char) -> c_int {
    let mut config = Config::default();
    // SAFETY: argc and argv should not be unsafe to dereference
    unsafe { config.apply_argv(argc, argv) };

    if cfg!(not(windows)) {
        config.error(|f| writeln!(f, "operating system `{}` is not supported", env::consts::OS));
        return 1;
    }

    #[cfg(windows)]
    {
        state::Storm::<
            backend::windows::WindowsBackendState,
            backend::windows::WindowsWindow,
            backend::windows::WindowsBackendError,
        >::new(config)
        .unwrap()
        .run()
        .unwrap();
    }

    0
}
