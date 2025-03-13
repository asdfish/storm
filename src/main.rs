#![no_main]

use std::{
    ffi::{
        CStr,
        c_char,
        c_int,
    },
    process::exit,
};

mod opts;

const EXIT_SUCCESS: c_int = 0;
const EXIT_FAILURE: c_int = 1;

// SAFETY: every c program has done this since the dawn of time
#[unsafe(no_mangle)]
fn main(argc: c_int, argv: *const *const c_char) -> c_int {
    if argc > 0 {
        (0..argc)
            // SAFETY: if argv and argc are unsafe, that is an operating system problem
            .map(|i| unsafe { argv.add(i.try_into().expect("argc should be filtered to be positive above")) })
            .filter(|ptr| !ptr.is_null())
            // SAFETY: null pointers are checked above
            .map(|arg| unsafe { CStr::from_ptr(*arg) })
            .filter_map(|arg| match arg.to_str() {
                Ok(arg) => Some(arg),
                Err(err) => {
                    todo!()
                }
            });
    }

    EXIT_SUCCESS
}
