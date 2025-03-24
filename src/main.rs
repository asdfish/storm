#![cfg_attr(not(test), no_main)]

mod backend;
mod bomb;
mod config;
mod const_string;
mod error;
mod iter_ext;
mod path_cache;
mod recursion;
mod state;

use {
    config::{
        ApplyError, Config,
        file_parser::{self, FileParser},
    },
    either::Either,
    path_cache::PathCache,
    path_cache::PathOrigin,
    std::{
        convert::Infallible,
        env,
        ffi::{c_char, c_int},
        fs::read_to_string,
    },
};

// Someone may be compiling without using cargo, so we cannot do `env!("CARGO_PKG_VERSION")`.
pub const NAME: &str = "storm";
pub const VERSION: &str = "0.1.0";

// SAFETY: every c program has done this since the dawn of time
#[cfg_attr(not(test), unsafe(no_mangle))]
fn main(argc: c_int, argv: *const *const c_char) -> c_int {
    let paths = PathCache::new();

    let mut config = Config::default();
    match unsafe { config.apply_argv(&paths, argc, argv) } {
        Ok(_) => {}
        Err(Either::Right(ApplyError::Exit)) => return 0,
        Err(err) => {
            eprintln!("error during argument parsing: {}", err);
            return 1;
        }
    }

    if let Some((path, origin)) = paths.get_config(&config) {
        'apply: {
            let contents = match read_to_string(path).map_err(move |err| (err, origin)) {
                Ok(contents) => Box::leak(file_parser::trim_string(contents)),
                Err((err, PathOrigin::Config)) => {
                    config.error(|f| {
                        writeln!(
                            f,
                            "failed to read configuration from path `{}`: {}",
                            path.display(),
                            err
                        )
                    });
                    return 1;
                }
                Err((_, PathOrigin::Default)) => break 'apply, // ignore default
            };

            if let Err(err) =
                config.apply_args(&paths, FileParser::new(contents).map(Ok::<_, Infallible>))
            {
                config.error(|f| writeln!(f, "error during argument parsing: {}", err));
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync() {
        macro_rules! check_sync {
            ($constant:ident, $env:literal) => {
                if let Some(env) = option_env!($env) {
                    assert_eq!(env, $constant);
                }
            };
        }

        check_sync!(NAME, "CARGO_PKG_NAME");
        check_sync!(VERSION, "CARGO_PKG_VERSION");
    }
}
