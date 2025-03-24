#![cfg_attr(not(test), no_main)]

mod backend;
mod bomb;
mod config;
mod const_string;
mod error;
mod recursion;
mod state;

use {
    config::{ApplyArgvError, ApplyError, Config},
    either::Either,
    std::{
        env,
        ffi::{c_char, c_int},
    },
};

// SAFETY: every c program has done this since the dawn of time
#[cfg_attr(not(test), unsafe(no_mangle))]
fn main(argc: c_int, argv: *const *const c_char) -> c_int {
    let mut config = Config::default();
    match unsafe { config.apply_argv(argc, argv) } {
        Ok(_) => {}
        Err(Either::Right(ApplyError::Exit)) => return 0,
        Err(err) => {
            eprintln!("error during argument parsing: {}", err);
            return 1;
        }
    }

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
