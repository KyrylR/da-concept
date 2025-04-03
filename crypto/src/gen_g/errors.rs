#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum GenGError {
    #[error("Input order k must be positive")]
    KMustBePositive,
    #[error("Input parameter l must be positive and divide k")]
    InvalidLValue,
    #[error("GCD(a, k) must be 1, but the calculated GCD was {0}")]
    GcdAKConstraintNotMet(i64),
    #[error("GCD(a, m) must be 1 (where m=k/l), but the calculated GCD was {0}")]
    GcdAMConstraintNotMet(i64),
    #[error("Internal error: Value 0 not found in sequence b")]
    ValueZeroNotFound,
    #[error("Internal error: Value 1 not found in sequence b")]
    ValueOneNotFound,
    #[error("Internal error: Overflow during calculation")]
    CalculationOverflow,
    #[error("Internal error: Invalid index {0} for P encountered from b")]
    IndexOutOfBounds(i64),
    #[error("Internal error: Construction of P failed unexpectedly")]
    ConstructionFailed,
}
