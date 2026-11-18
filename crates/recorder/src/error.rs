use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum RecorderError {
    #[error("no microphone available")]
    NoInputDevice,
    #[error("microphone {0:?} was not found")]
    DeviceNotFound(String),
    #[error("microphone is not available: {0}")]
    Device(String),
    #[error("the microphone does not support this format: {0}")]
    UnsupportedFormat(String),
    #[error("could not start recording: {0}")]
    Start(String),
    #[error("recording file error at {path}: {source}")]
    File {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("recording thread stopped unexpectedly: {0}")]
    Thread(String),
}

pub type Result<T> = std::result::Result<T, RecorderError>;

pub(crate) fn file_error(path: &std::path::Path, source: std::io::Error) -> RecorderError {
    RecorderError::File {
        path: path.to_path_buf(),
        source,
    }
}
