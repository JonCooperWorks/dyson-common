//! Shared Telegram primitives for Dyson runtimes.

pub mod media;

#[cfg(feature = "file-api")]
mod file_api;
#[cfg(feature = "file-api")]
pub use file_api::{FileDownloadError, download_file};
