//! Token scanner for the A2L grammar.
//!
//! Tokens: identifiers/keywords (`CHARACTERISTIC`, `RL_LINEARDT`, …),
//! slash-prefixed keywords (`/begin`, `/end`), numeric literals (decimal,
//! hex `0x…`, float with exponent), quoted strings with C-style escapes,
//! punctuation (`{`, `}`, `=`, … — skipped by the parser), and comments
//! (`/* … */`, `// … \n`) which the scanner drops. Every byte access goes
//! through `get()` — the scanner is total by construction.
use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::vec::Vec;

use crate::error::A2lError;

/// One scanned token with its 1-based source line.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Token {
    /// Token kind and payload.
    pub kind: TokenKind,
    /// 1-based line the token starts on.
    pub line: usize,
}

/// Token kinds.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum TokenKind {
    /// Bare identifier or keyword (`CHARACTERISTIC`, `UBYTE`, …).
    Ident(String),
    /// Slash-prefixed keyword (`/begin`, `/end`); payload excludes the
    /// slash.
    Slash(String),
    /// Numeric literal, already parsed.
    Number(f64),
    /// Quoted string, escapes decoded.
    Str(String),
    /// Any other single character (`{`, `}`, `=`, `,`, …). Skipped by the
    /// parser; kept so hostile bytes cannot desynchronize block counting.
    Punct(char),
}

/// Scan the whole input into tokens.
///
/// One state machine over the byte stream by design — splitting the
/// number/string/keyword arms into helpers would scatter the shared
/// cursor and line-counter invariants.
#[allow(clippy::too_many_lines)]
pub(crate) fn lex(input: &str) -> Result<Vec<Token>, A2lError> {
    let bytes = input.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0usize;
    let mut line = 1usize;

    while let Some(&c) = bytes.get(i) {
        match c {
            b'\n' => {
                line += 1;
                i += 1;
            }
            b' ' | b'\t' | b'\r' | 0x0B | 0x0C => i += 1,
            b'/' => {
                let next = bytes.get(i + 1).copied();
                match next {
                    None => i += 1,
                    Some(b'*') => {
                        // Block comment (non-nesting, C-style).
                        let start_line = line;
                        let mut j = i + 2;
                        loop {
                            match bytes.get(j).copied() {
                                None => {
                                    return Err(A2lError::UnterminatedComment { line: start_line })
                                }
                                Some(b'\n') => {
                                    line += 1;
                                    j += 1;
                                }
                                Some(b'*') if bytes.get(j + 1) == Some(&b'/') => {
                                    j += 2;
                                    break;
                                }
                                Some(_) => j += 1,
                            }
                        }
                        i = j;
                    }
                    Some(b'/') => {
                        // Line comment: drop to end of line (the newline
                        // itself is consumed by the whitespace arm).
                        while bytes.get(i).copied().is_some_and(|b| b != b'\n') {
                            i += 1;
                        }
                    }
                    Some(_) => {
                        // Slash-prefixed keyword: /begin, /end.
                        let start = i + 1;
                        let mut j = start;
                        while bytes.get(j).copied().is_some_and(is_ident_byte) {
                            j += 1;
                        }
                        if j > start {
                            tokens.push(Token {
                                kind: TokenKind::Slash(input[start..j].to_owned()),
                                line,
                            });
                        } else {
                            tokens.push(Token {
                                kind: TokenKind::Punct('/'),
                                line,
                            });
                        }
                        i = j.max(start);
                    }
                }
            }
            b'"' => {
                let start_line = line;
                let mut text = String::new();
                let mut j = i + 1;
                loop {
                    let Some(&b) = bytes.get(j) else {
                        return Err(A2lError::UnterminatedString { line: start_line });
                    };
                    match b {
                        b'\n' => {
                            line += 1;
                            j += 1;
                        }
                        b'"' => {
                            j += 1;
                            break;
                        }
                        b'\\' => {
                            // C-style escapes; unknown escapes keep the
                            // backslash verbatim and continue at the next
                            // byte (which may be a multi-byte UTF-8
                            // sequence — never slice mid-character).
                            let Some(&esc) = bytes.get(j + 1) else {
                                return Err(A2lError::UnterminatedString { line: start_line });
                            };
                            match esc {
                                b'n' => {
                                    text.push('\n');
                                    j += 2;
                                }
                                b't' => {
                                    text.push('\t');
                                    j += 2;
                                }
                                b'r' => {
                                    text.push('\r');
                                    j += 2;
                                }
                                b'"' => {
                                    text.push('"');
                                    j += 2;
                                }
                                b'\\' => {
                                    text.push('\\');
                                    j += 2;
                                }
                                _ => {
                                    text.push('\\');
                                    j += 1;
                                }
                            }
                        }
                        _ if b < 128 => {
                            // Copy the ASCII byte run in one go (run ends
                            // are ASCII by construction).
                            let run_start = j;
                            while bytes
                                .get(j)
                                .copied()
                                .is_some_and(|b| b < 128 && b != b'"' && b != b'\\' && b != b'\n')
                            {
                                j += 1;
                            }
                            text.push_str(&input[run_start..j]);
                        }
                        _ => {
                            // Multi-byte UTF-8: decode one char from the
                            // str (j is always on a char boundary here).
                            if let Some(ch) = input[j..].chars().next() {
                                text.push(ch);
                                j += ch.len_utf8();
                            } else {
                                j += 1;
                            }
                        }
                    }
                }
                tokens.push(Token {
                    kind: TokenKind::Str(text),
                    line: start_line,
                });
                i = j;
            }
            b'0'..=b'9' | b'-' | b'+' => {
                let start = i;
                let signed = c == b'-' || c == b'+';
                let after_sign = if signed { i + 1 } else { i };
                let is_hex = bytes.get(after_sign).copied() == Some(b'0')
                    && matches!(bytes.get(after_sign + 1).copied(), Some(b'x' | b'X'))
                    && bytes
                        .get(after_sign + 2)
                        .copied()
                        .is_some_and(|b| b.is_ascii_hexdigit());
                if is_hex {
                    // Hex literal: [sign] 0x digits.
                    let digits_start = after_sign + 2;
                    let mut j = digits_start;
                    while bytes.get(j).copied().is_some_and(|b| b.is_ascii_hexdigit()) {
                        j += 1;
                    }
                    let digits = &input[digits_start..j];
                    // u64 then f64: hex addresses up to u32::MAX are exact
                    // in f64; 64-bit literals degrade gracefully.
                    let value =
                        u64::from_str_radix(digits, 16).map_err(|_| A2lError::InvalidNumber {
                            text: input[start..j].to_owned(),
                            line,
                        })?;
                    // Addresses ≤ u32::MAX are exact in f64; wider hex
                    // literals degrade gracefully (documented).
                    #[allow(clippy::cast_precision_loss)]
                    let value = value as f64;
                    tokens.push(Token {
                        kind: TokenKind::Number(value),
                        line,
                    });
                    i = j;
                } else {
                    // Decimal / float: consume the maximal numeric run —
                    // digits, dots, exponents (with their sign).
                    let mut j = after_sign;
                    let mut allow_dot = true;
                    while let Some(&b) = bytes.get(j) {
                        if b.is_ascii_digit() {
                            j += 1;
                        } else if b == b'.' && allow_dot {
                            allow_dot = false;
                            j += 1;
                        } else if (b == b'e' || b == b'E')
                            && j > after_sign
                            && bytes
                                .get(j - 1)
                                .copied()
                                .is_some_and(|p| p.is_ascii_digit() || p == b'.')
                            && bytes
                                .get(j + 1)
                                .copied()
                                .is_some_and(|n| n.is_ascii_digit() || n == b'-' || n == b'+')
                        {
                            j += 2; // consume 'e' and its sign/digit start
                        } else if (b == b'-' || b == b'+')
                            && j == after_sign
                            && bytes.get(j + 1).copied() == Some(b'.')
                        {
                            j += 1; // sign immediately before a leading dot
                        } else {
                            break;
                        }
                    }
                    if j == after_sign {
                        // A lone sign with no digits: punctuation junk.
                        tokens.push(Token {
                            kind: TokenKind::Punct(char::from(c)),
                            line,
                        });
                        i += 1;
                    } else {
                        let text = &input[start..j];
                        let value: f64 = text.parse().map_err(|_| A2lError::InvalidNumber {
                            text: text.to_owned(),
                            line,
                        })?;
                        tokens.push(Token {
                            kind: TokenKind::Number(value),
                            line,
                        });
                        i = j;
                    }
                }
            }
            _ if is_ident_start(c) => {
                let start = i;
                let mut j = i;
                while bytes.get(j).copied().is_some_and(is_ident_byte) {
                    j += 1;
                }
                tokens.push(Token {
                    kind: TokenKind::Ident(input[start..j].to_owned()),
                    line,
                });
                i = j;
            }
            _ => {
                tokens.push(Token {
                    kind: TokenKind::Punct(char::from(c)),
                    line,
                });
                i += 1;
            }
        }
    }
    Ok(tokens)
}

const fn is_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_'
}

const fn is_ident_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}
