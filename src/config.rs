use std::{env, path::PathBuf};

pub(crate) fn config_dir() -> Option<PathBuf> {
    if let Some(path) = env::var_os("TUICORE_CONFIG_DIR") {
        return Some(PathBuf::from(path));
    }

    let home = env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".tuicore"))
}
