//! # Crypto
//!
//! This library provides the foundational mathematical structures based on the paper
//! "Симетрична криптосистема на основі відображень кілець".

pub mod gen_g;
pub mod ring;
pub mod sle;
pub mod system;

pub use ring::*;
