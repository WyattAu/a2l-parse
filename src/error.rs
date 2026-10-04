//! Typed, exhaustive error taxonomy for A2L parsing.
//!
//! The enum is deliberately **exhaustive** (no `#[non_exhaustive]`):
//! callers match every variant, and a new variant is a semver-minor event
//! by design — the same policy as `can-core`'s `CanError` and `xcp-core`'s
//! `XcpError`. Every malformed input resolves to one of these variants,
//! never a panic.
use alloc::string::String;

use core::fmt;

/// The single crate-level error for lexing and parsing.
#[derive(Debug, Clone, PartialEq)]
pub enum A2lError {
    /// A quoted string runs to end of input without a closing quote.
    UnterminatedString {
        /// Line the string started on (1-based).
        line: usize,
    },
    /// A `/* */` comment runs to end of input without `*/`.
    UnterminatedComment {
        /// Line the comment started on (1-based).
        line: usize,
    },
    /// A numeric literal is not a valid number (includes hex literals
    /// with bad digits and malformed floats).
    InvalidNumber {
        /// The offending text.
        text: String,
        /// Line the literal starts on (1-based).
        line: usize,
    },
    /// A number that must be a 32-bit ECU address is negative or exceeds
    /// `u32::MAX`.
    InvalidAddress {
        /// The offending value.
        value: f64,
        /// Line the literal starts on (1-based).
        line: usize,
    },
    /// Input ended where a token was required.
    UnexpectedEof {
        /// What the parser was looking for.
        context: &'static str,
    },
    /// A token of a different kind appeared where one was required.
    UnexpectedToken {
        /// What the parser required (human-readable).
        expected: &'static str,
        /// What was found.
        got: String,
        /// Line of the offending token (1-based).
        line: usize,
    },
    /// A `DATATYPE` keyword outside the ASAP2 data-type set.
    InvalidDatatype {
        /// The offending keyword.
        text: String,
        /// Line (1-based).
        line: usize,
    },
    /// A CHARACTERISTIC type keyword outside the ASAP2 set (VALUE, CURVE,
    /// MAP, VAL_BLK, ASCII).
    InvalidCharType {
        /// The offending keyword.
        text: String,
        /// Line (1-based).
        line: usize,
    },
    /// A COMPU_METHOD conversion type outside the supported set (RAT_FUNC,
    /// LINEAR, TABLE, IDENTITY/IDENTICAL).
    InvalidConversionType {
        /// The offending keyword.
        text: String,
        /// Line (1-based).
        line: usize,
    },
    /// A `/begin NAME` block ran to end of input without its `/end NAME`.
    UnterminatedBlock {
        /// The block keyword.
        block: String,
        /// Line the `/begin` appeared on (1-based).
        line: usize,
    },
    /// A `/end NAME` closed a different block than the innermost open one.
    BlockMismatch {
        /// The innermost open block keyword.
        opened: String,
        /// The `/end` keyword.
        closed: String,
        /// Line of the `/end` (1-based).
        line: usize,
    },
    /// No `/begin PROJECT` block was found at the top level.
    MissingProject,
    /// `/begin` blocks nested deeper than the parser's limit
    /// ([`MAX_DEPTH`](crate::MAX_DEPTH)) — a guard against pathological
    /// input, never triggered by conforming files.
    DepthLimitExceeded {
        /// Line of the offending `/begin` (1-based).
        line: usize,
    },
}

impl A2lError {
    /// The 1-based line the error was detected on, when applicable.
    #[must_use]
    pub fn line(&self) -> Option<usize> {
        Some(match self {
            Self::UnterminatedString { line }
            | Self::UnterminatedComment { line }
            | Self::InvalidNumber { line, .. }
            | Self::InvalidAddress { line, .. }
            | Self::UnexpectedToken { line, .. }
            | Self::InvalidDatatype { line, .. }
            | Self::InvalidCharType { line, .. }
            | Self::InvalidConversionType { line, .. }
            | Self::UnterminatedBlock { line, .. }
            | Self::BlockMismatch { line, .. }
            | Self::DepthLimitExceeded { line } => *line,
            Self::UnexpectedEof { .. } | Self::MissingProject => return None,
        })
    }
}

impl fmt::Display for A2lError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnterminatedString { line } => write!(f, "line {line}: unterminated string"),
            Self::UnterminatedComment { line } => write!(f, "line {line}: unterminated /* comment"),
            Self::InvalidNumber { text, line } => {
                write!(f, "line {line}: invalid number `{text}`")
            }
            Self::InvalidAddress { value, line } => {
                write!(f, "line {line}: value {value} is not a 32-bit ECU address")
            }
            Self::UnexpectedEof { context } => {
                write!(f, "unexpected end of input: expected {context}")
            }
            Self::UnexpectedToken {
                expected,
                got,
                line,
            } => {
                write!(f, "line {line}: expected {expected}, found `{got}`")
            }
            Self::InvalidDatatype { text, line } => {
                write!(f, "line {line}: unknown ASAP2 datatype `{text}`")
            }
            Self::InvalidCharType { text, line } => {
                write!(f, "line {line}: unknown CHARACTERISTIC type `{text}`")
            }
            Self::InvalidConversionType { text, line } => {
                write!(
                    f,
                    "line {line}: unknown COMPU_METHOD conversion type `{text}`"
                )
            }
            Self::UnterminatedBlock { block, line } => {
                write!(
                    f,
                    "line {line}: `/begin {block}` without matching `/end {block}`"
                )
            }
            Self::BlockMismatch {
                opened,
                closed,
                line,
            } => {
                write!(f, "line {line}: `/end {closed}` closes `/begin {opened}`")
            }
            Self::MissingProject => f.write_str("no `/begin PROJECT` block found"),
            Self::DepthLimitExceeded { line } => {
                write!(f, "line {line}: block nesting exceeds the parser limit")
            }
        }
    }
}

impl core::error::Error for A2lError {}
