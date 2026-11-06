use std::path::PathBuf;

use thiserror::Error;

use crate::core::crop::PixelRect;

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("not a recognised image format")]
    UnknownFormat,
    #[error("cannot decode image: {0}")]
    Image(#[from] image::ImageError),
    #[error(
        "image is {width}x{height}, which needs more than the {limit_mib} MiB decoded size limit"
    )]
    TooLarge {
        width: usize,
        height: usize,
        limit_mib: u64,
    },
    #[error("pixel buffer does not match {width}x{height} RGBA")]
    PixelBufferMismatch { width: usize, height: usize },
}

#[derive(Debug, Error)]
pub enum EncodeError {
    #[error("cannot encode image: {0}")]
    Image(#[from] image::ImageError),
    #[error("cannot encode image: {0}")]
    Backend(&'static str),
}

#[derive(Debug, Error)]
pub enum ResampleError {
    #[error("cannot resize to {width}x{height}")]
    EmptyTarget { width: usize, height: usize },
    #[error(transparent)]
    TooLarge(#[from] DecodeError),
    #[error("source pixels are unusable: {0}")]
    Source(#[from] fast_image_resize::ImageBufferError),
    #[error("cannot resize image: {0}")]
    Resize(#[from] fast_image_resize::ResizeError),
}

#[derive(Debug, Error)]
pub enum CropError {
    #[error("crop area {rect:?} lies outside the {}x{} image", .size[0], .size[1])]
    OutsideImage { rect: PixelRect, size: [usize; 2] },
    #[error("pixel buffer does not match {}x{} RGBA", .size[0], .size[1])]
    SourceMismatch { size: [usize; 2] },
}

#[derive(Debug, Error)]
pub enum ReplayError {
    #[error(transparent)]
    Resample(#[from] ResampleError),
    #[error(transparent)]
    Crop(#[from] CropError),
}

#[derive(Debug, Error)]
pub enum JobError {
    #[error(transparent)]
    Platform(#[from] PlatformError),
    #[error(transparent)]
    Resample(#[from] ResampleError),
    #[error(transparent)]
    Crop(#[from] CropError),
    #[error(transparent)]
    Replay(#[from] ReplayError),
    #[error(transparent)]
    Encode(#[from] EncodeError),
    #[error("cannot save as .{extension}; use png, jpg, webp or bmp")]
    UnsupportedExtension { extension: String },
    #[error("cannot write {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("the background task stopped unexpectedly")]
    Stopped,
}

#[derive(Debug, Error)]
pub enum FetchError {
    #[error("request failed: {0}")]
    Transport(#[from] ureq::Error),
    #[error("download is larger than the {limit} byte limit")]
    TooLarge { limit: u64 },
    #[error("download interrupted: {0}")]
    Interrupted(#[from] std::io::Error),
    #[error("request cancelled")]
    Cancelled,
}

#[derive(Debug, Error)]
pub enum IngestError {
    #[error("nothing here looks like an image, a file path or a web link")]
    NothingRecognised,
    #[error("cannot read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{path} is not a regular file")]
    NotAFile { path: PathBuf },
    #[error("{size} bytes exceeds the {limit} byte input limit")]
    InputTooLarge { size: u64, limit: u64 },
    #[error("malformed data URI")]
    MalformedDataUri,
    #[error(transparent)]
    Fetch(#[from] FetchError),
    #[error(transparent)]
    Decode(#[from] DecodeError),
    #[error(transparent)]
    Platform(#[from] PlatformError),
}

#[derive(Debug, Error)]
pub enum PlatformError {
    #[error("{0}")]
    Unsupported(&'static str),
    #[error("the drag source stopped responding")]
    DragSourceStalled,
    #[error("dragged data is larger than the {limit} byte limit")]
    DragDataTooLarge { limit: u64 },
    #[error("cannot read the clipboard: {0}")]
    Clipboard(#[from] arboard::Error),
    #[error("cannot write the clipboard: {0}")]
    ClipboardWrite(arboard::Error),
    #[cfg(target_os = "macos")]
    #[error("{0}")]
    WindowUnavailable(&'static str),
    #[cfg(windows)]
    #[error("window system request failed: {0}")]
    Win32(#[from] std::io::Error),
    #[cfg(all(unix, not(target_os = "macos")))]
    #[error("cannot connect to the X server: {0}")]
    X11Connect(#[from] x11rb::errors::ConnectError),
    #[cfg(all(unix, not(target_os = "macos")))]
    #[error("X server connection failed: {0}")]
    X11Connection(#[from] x11rb::errors::ConnectionError),
    #[cfg(all(unix, not(target_os = "macos")))]
    #[error("cannot allocate an X resource id: {0}")]
    X11Id(#[from] x11rb::errors::ReplyOrIdError),
    #[cfg(all(unix, not(target_os = "macos")))]
    #[error("X server rejected the request: {0}")]
    X11Reply(#[from] x11rb::errors::ReplyError),
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("this system has no configuration directory")]
    NoDirectory,
    #[error("cannot access {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{path} is not valid TOML: {}", .source.message())]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("cannot encode settings: {0}")]
    Encode(#[from] toml::ser::Error),
}

#[derive(Debug, Error)]
pub enum UpdateError {
    #[error("cannot reach the update server: {0}")]
    Fetch(#[from] FetchError),
    #[error("the update information is not valid TOML: {}", .0.message())]
    Manifest(#[from] toml::de::Error),
    #[error("the update information has no usable version")]
    NoVersion,
    #[error("this release has no signed build for this system")]
    NoBuild,
    #[error("this build cannot verify updates because it has no signing key")]
    NoKey,
    #[error("the update signature is not valid")]
    BadSignature,
    #[error("cannot replace {path}: {source}")]
    Replace {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Debug, Error)]
pub enum InstallError {
    #[error("{0}")]
    Unsupported(&'static str),
    #[error("this system has no home directory")]
    NoDirectory,
    #[error("komaru is not installed")]
    NotInstalled,
    #[error("cannot write {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}
