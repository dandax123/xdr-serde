use serde::{de, ser};
use std::fmt;

pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur during XDR serialization or deserialization.
#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// A custom error message from serde
    Message(String),

    /// Attempted to read past the end of the input buffer
    UnexpectedEof,

    /// A sequence or map length was not known ahead of time (XDR requires it)
    LengthRequired,

    /// A string contained non-ASCII or non-UTF-8 bytes
    InvalidString,

    /// The discriminant value for a union/enum is not valid
    InvalidDiscriminant(i32),

    /// The boolean encoding was neither 0 nor 1
    InvalidBool(u32),

    /// An optional value had an invalid discriminant (must be 0 or 1)
    InvalidOption(u32),

    /// Data exceeded the declared maximum length
    LengthOverflow { max: u32, got: u32 },

    /// Padding bytes were non-zero (strict mode violation)
    InvalidPadding,

    /// XDR does not support this serde data model type
    Unsupported(&'static str),

    /// An I/O error occurred during writing
    Io(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Message(msg) => write!(f, "{}", msg),
            Error::UnexpectedEof => write!(f, "unexpected end of input"),
            Error::LengthRequired => {
                write!(
                    f,
                    "sequence length must be known before serialization (XDR requires a length prefix)"
                )
            }
            Error::InvalidString => write!(f, "string contains invalid bytes"),
            Error::InvalidDiscriminant(v) => write!(f, "invalid discriminant value: {}", v),
            Error::InvalidBool(v) => write!(f, "invalid boolean encoding: {} (must be 0 or 1)", v),
            Error::InvalidOption(v) => {
                write!(f, "invalid optional discriminant: {} (must be 0 or 1)", v)
            }
            Error::LengthOverflow { max, got } => {
                write!(f, "length {} exceeds maximum {}", got, max)
            }
            Error::InvalidPadding => write!(f, "non-zero padding bytes"),
            Error::Unsupported(t) => write!(f, "XDR does not support type: {}", t),
            Error::Io(msg) => write!(f, "I/O error: {}", msg),
        }
    }
}

impl std::error::Error for Error {}

impl ser::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Error::Message(msg.to_string())
    }
}

impl de::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Error::Message(msg.to_string())
    }
}
