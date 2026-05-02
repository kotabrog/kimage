//! A small image IO crate built step by step with the Rust standard library.

mod error;
mod image;
pub mod io;

pub use error::{ImageError, Result};
pub use image::{Image, ImageView, PixelFormat};
