use crate::RingError;
use crate::gen_g::GenGError;
use crate::sle::SleError;

#[derive(thiserror::Error, Debug)]
pub enum CryptoError {
    // Placeholder errors assuming submodules define these types:
    #[error("GenG error: {0}")]
    GenG(#[from] GenGError),
    #[error("Ring error: {0}")]
    Ring(#[from] RingError),
    #[error("SLE error: {0}")]
    Sle(#[from] SleError),

    #[error("Dimension mismatch: {0}")]
    DimensionMismatch(String),
    #[error("Matrix is singular (mod m)")]
    MatrixSingular,
    #[error("Failed to find unique solution for SLE")]
    SleNoUniqueSolution,
    #[error("Invalid parameters: {0}")]
    InvalidParameters(String),
    #[error("Calculation overflow")]
    Overflow,
    #[error("Isomorphism mapping error: {0}")]
    IsomorphismError(String),
    #[error("Random number generation error: {0}")]
    RandError(String),
    #[error("Value not found: {0}")]
    ValueNotFound(String),
    #[error("Setup error: {0}")]
    SetupError(String),
}
