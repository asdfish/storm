#![cfg_attr(not(test), no_main)]

use {
    std::{
        env,
        ffi::{c_char, c_int},
    },
    storm::config::{ApplyArgvError, ApplyError, Config},
};

// SAFETY: every c program has done this since the dawn of time
#[cfg_attr(not(test), unsafe(no_mangle))]
fn main(argc: c_int, argv: *const *const c_char) -> c_int {
    let mut config = Config::default();
    match unsafe { config.apply_argv(argc, argv) } {
        Ok(_) => {}
        Err(ApplyArgvError::Apply(ApplyError::Exit)) => return 0,
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
        storm::state::Storm::<
            storm::backend::windows::WindowsBackendState,
            storm::backend::windows::WindowsWindow,
            storm::backend::windows::WindowsBackendError,
        >::new(config)
        .unwrap()
        .run()
        .unwrap();
    }

    0
}
