use std::fmt;

use std::io;
use thiserror::Error;

/// WIP
#[non_exhaustive]
#[derive(Debug, Error)]
pub enum StructureError {
    /// Io related error
    #[error(transparent)]
    Io { source: io::Error },
    /// Csv related error
    /// Luci custom error that describes errors in usage
    #[error("Missing a value")]
    MissingValue,
    #[error("Invalid state")]
    InvalidState,
    #[error("Unsupported csv structure: likely variable number of fields")]
    InvalidCsvFormat,
}
//------------------------------------------------------------------------------
// Error implementation
//------------------------------------------------------------------------------
// impl Error for InspectionError {}

//------------------------------------------------------------------------------
// From implementation
//------------------------------------------------------------------------------

impl From<io::Error> for StructureError {
    fn from(err: io::Error) -> StructureError {
        StructureError::Io { source: err }
    }
}
impl<T> From<std::result::Result<T, StructureError>> for StructureError {
    fn from(err: std::result::Result<T, StructureError>) -> StructureError {
        StructureError::InvalidState
    }
}
//------------------------------------------------------------------------------
// Text parsing errors (copy/paste from json?)
//------------------------------------------------------------------------------
/// Error types encountered while parsing
#[derive(Debug)]
pub enum ErrorType {
    /// only supports inputs of up to
    /// 4GB in size.
    InputTooLarge,
    /// The data ended early
    EarlyEnd,
    /// Internal error
    InternalError,
    /// Invalid escape sequence
    InvalidEscape,
    /// Invalid number
    InvalidNumber,
    /// Inbalid UTF8 codepoint
    InvalidUTF8,
    /// Invalid Unicode escape sequence
    InvalidUnicodeEscape,
    /// Inbalid Unicode codepoint
    InvlaidUnicodeCodepoint,
    /// Non structural character
    NoStructure,
    /// Parser Erropr
    Parser,
    /// Early End Of File
    EOF,
    /// Unexpected end
    UnexpectedEnd,
    /// Unterminated string
    UnterminatedString,
    /// Overflow of a limited buffer
    Overflow,
    /// Generic syntax
    Syntax,
    /// IO error
    IO(std::io::Error),
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::generic(ErrorType::IO(e))
    }
}

#[cfg(not(tarpaulin_include))]
impl PartialEq for ErrorType {
    #[must_use]
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::IO(_), Self::IO(_))
            | (Self::EarlyEnd, Self::EarlyEnd)
            | (Self::InternalError, Self::InternalError)
            | (Self::InvalidEscape, Self::InvalidEscape)
            | (Self::InvalidNumber, Self::InvalidNumber)
            | (Self::InvalidUTF8, Self::InvalidUTF8)
            | (Self::InvalidUnicodeEscape, Self::InvalidUnicodeEscape)
            | (Self::InvlaidUnicodeCodepoint, Self::InvlaidUnicodeCodepoint)
            | (Self::Parser, Self::Parser)
            | (Self::EOF, Self::EOF)
            | (Self::Syntax, Self::Syntax)
            | (Self::UnterminatedString, Self::UnterminatedString)
            | (Self::Overflow, Self::Overflow) => true,
            _ => false,
        }
    }
}
/// Parser error
#[derive(Debug, PartialEq)]
pub struct Error {
    /// Byte index it was encountered at
    index: usize,
    /// Current character
    character: char,
    /// Tyep of error
    error: ErrorType,
}

impl Error {
    pub(crate) fn new(index: usize, character: char, error: ErrorType) -> Self {
        Self {
            index,
            character,
            error,
        }
    }
    /// Create a generic error
    #[must_use = "Error creation"]
    pub fn generic(t: ErrorType) -> Self {
        Self {
            index: 0,
            character: '💩', //this is the poop emoji
            error: t,
        }
    }
}
impl std::error::Error for Error {}

#[cfg(not(tarpaulin_include))]
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{:?} at character {} ('{}')",
            self.error, self.index, self.character
        )
    }
}

#[cfg(not(tarpaulin_include))]
impl From<Error> for std::io::Error {
    fn from(e: Error) -> Self {
        std::io::Error::new(std::io::ErrorKind::InvalidData, e)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn fmt() {
        let e = Error::generic(ErrorType::InternalError);
        assert_eq!(
            format!("{}", e),
            "InternalError at character 0 ('\u{1f4a9}')"
        )
    }
}
