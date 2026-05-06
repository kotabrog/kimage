//! A small image IO crate built step by step with the Rust standard library.

pub mod codecs;
mod error;
mod format;
mod image;
mod io;

pub use error::{ImageError, Result};
pub use format::decode;
pub use image::{Image, ImageView, PixelFormat};
