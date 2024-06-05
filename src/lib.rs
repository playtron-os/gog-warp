pub mod auth;
pub mod constants;
pub mod content_system;
mod core;
pub mod errors;
pub mod gameplay;
pub mod library;
pub mod user;
pub mod utils;

#[allow(dead_code)]
#[cfg(feature = "downloader")]
mod xdelta;

pub use crate::errors::Error;
pub use content_system::types::Platform;
pub use core::Core;

#[cfg(feature = "downloader")]
pub use content_system::downloader::Downloader;
