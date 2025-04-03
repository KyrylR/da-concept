//! # Ring Crypto Module
//!
//! It includes the implementation for the finite ring Z_k

pub mod errors;
pub mod helper;
pub mod math;

pub use errors::RingError;
pub use helper::extended_gcd;
pub use math::Ring;
