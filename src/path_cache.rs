use {
    crate::{
        config::Config,
        NAME,
    },
    directories::BaseDirs,
    std::{
        borrow::Cow,
        cell::LazyCell,
        path::{Path, PathBuf},
    },
};

pub struct PathCache {
    pub config: LazyCell<Option<PathBuf>>
}
impl PathCache {
    pub const fn new() -> Self {
        Self {
            config: LazyCell::new(|| {
                BaseDirs::new()
                    .map(|dirs| {
                        const CONFIG_FILE: &str = "config.txt";

                        let mut config_path = dirs.config_dir().to_path_buf();
                        config_path.reserve_exact(NAME.len() + 1 + CONFIG_FILE.len());
                        config_path.push(NAME);
                        config_path.push(CONFIG_FILE);
                        config_path.shrink_to_fit();
                        config_path
                    })
            })
        }
    }

    pub fn get_config<'a>(&'a self, config: &Config<'a>) -> Option<&'a Path> {
        config
            .config_file
            .or(self.config.as_ref().map(|path| path.as_path()))
    }
}
