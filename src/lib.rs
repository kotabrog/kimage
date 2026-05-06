//! A small image IO crate built step by step with the Rust standard library.

pub mod codecs;
mod error;
mod format;
mod image;
mod io;

pub use error::{ImageError, Result};
pub use format::{NativeImage, decode, decode_native};
pub use image::{Image, ImageView, PixelFormat};
