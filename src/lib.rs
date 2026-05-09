//! A small image IO crate built step by step with the Rust standard library.

pub mod codecs;
mod error;
mod format;
mod image;
#[cfg(feature = "bmp")]
mod io;

pub use error::{ImageError, Result};
pub use format::{
    EncodeFormat, NativeImage, decode, decode_all_native, decode_native, encode, encode_all_native,
    encode_native,
};
pub use image::{Image, ImageView, PixelFormat};
