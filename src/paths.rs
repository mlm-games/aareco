use std::path::PathBuf;

pub fn plugins_dir() -> PathBuf {
    data_root().join("plugins")
}

#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
pub fn data_root() -> PathBuf {
    if let Ok(root) = std::env::var("AARECO_DATA_ROOT")
        && !root.trim().is_empty()
    {
        return PathBuf::from(root);
    }
    directories::ProjectDirs::from("app", "aareco", "aareco")
        .map(|dirs| dirs.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("aareco-data"))
}

#[cfg(target_os = "android")]
static ANDROID_DATA_ROOT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

#[cfg(target_os = "android")]
pub fn set_android_data_root(path: PathBuf) {
    let _ = ANDROID_DATA_ROOT.set(path);
}

#[cfg(target_os = "android")]
pub fn data_root() -> PathBuf {
    ANDROID_DATA_ROOT
        .get()
        .cloned()
        .unwrap_or_else(|| PathBuf::from("aareco-data"))
}

#[cfg(target_arch = "wasm32")]
pub fn data_root() -> PathBuf {
    PathBuf::from("aareco-data")
}
