//! Recursive-descent parser for the block-structured ASAP2 grammar.
//!
//! The parser is **keyword-driven and skip-tolerant**: within every known
//! block it consumes the required positional parameters, captures the
//! optional keywords the model needs (`ECU_ADDRESS`, `COEFFS`,
//! `COEFFS_LINEAR`, `COMPU_TAB_REF`, `FNC_VALUES`), and skips every other
//! token — including balanced unknown `/begin … /end` sub-blocks
//! (AXIS_DESCR, IF_DATA, FUNCTION, vendor extensions). This mirrors how
//! production A2L tooling survives real-world files.
use alloc::borrow::ToOwned;
use alloc::format;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;

use crate::error::A2lError;
use crate::lexer::{lex, Token, TokenKind};
use crate::model::{
    A2lModule, A2lProject, CharType, Characteristic, Coeffs, CompuMethod, ConversionType, DataType,
    FncValues, Measurement, RecordLayout,
};

/// Maximum `/begin` nesting depth — a pathological-input guard, far above
/// anything a conforming file nests.
pub(crate) const MAX_DEPTH: usize = 32;

/// Parse a whole A2L document into the data model.
pub(crate) fn parse(input: &str) -> Result<A2lProject, A2lError> {
    let tokens = lex(input)?;
    Parser::new(&tokens).parse_project()
}

struct Parser<'t> {
    tokens: &'t [Token],
    pos: usize,
}

impl<'t> Parser<'t> {
    fn new(tokens: &'t [Token]) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&'t Token> {
        self.tokens.get(self.pos)
    }

    fn bump(&mut self) -> Option<&'t Token> {
        let tok = self.tokens.get(self.pos)?;
        self.pos += 1;
        Some(tok)
    }

    fn at_end(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    /// Consume `/begin NAME`, returning `NAME`.
    fn expect_begin(&mut self) -> Result<(&'t str, usize), A2lError> {
        let tok = self
            .bump()
            .ok_or(A2lError::UnexpectedEof { context: "/begin" })?;
        let TokenKind::Slash(word) = &tok.kind else {
            return Err(A2lError::UnexpectedToken {
                expected: "/begin",
                got: render(tok),
                line: tok.line,
            });
        };
        if word != "begin" {
            return Err(A2lError::UnexpectedToken {
                expected: "/begin",
                got: render(tok),
                line: tok.line,
            });
        }
        let name_tok = self.bump().ok_or(A2lError::UnexpectedEof {
            context: "block keyword after /begin",
        })?;
        let TokenKind::Ident(name) = &name_tok.kind else {
            return Err(A2lError::UnexpectedToken {
                expected: "block keyword after /begin",
                got: render(name_tok),
                line: name_tok.line,
            });
        };
        Ok((name.as_str(), tok.line))
    }

    /// Consume the block-name identifier after a `/end` whose slash token
    /// has already been consumed, returning `NAME` and line.
    fn end_name(&mut self) -> Result<(&'t str, usize), A2lError> {
        let tok = self.bump_or("block keyword after /end")?;
        match &tok.kind {
            TokenKind::Ident(name) => Ok((name.as_str(), tok.line)),
            _ => Err(A2lError::UnexpectedToken {
                expected: "block keyword after /end",
                got: render(tok),
                line: tok.line,
            }),
        }
    }

    fn bump_or(&mut self, context: &'static str) -> Result<&'t Token, A2lError> {
        self.bump().ok_or(A2lError::UnexpectedEof { context })
    }

    /// Consume an identifier token.
    fn expect_ident(&mut self, context: &'static str) -> Result<&'t str, A2lError> {
        let tok = self.bump_or(context)?;
        match &tok.kind {
            TokenKind::Ident(name) => Ok(name.as_str()),
            _ => Err(A2lError::UnexpectedToken {
                expected: context,
                got: render(tok),
                line: tok.line,
            }),
        }
    }

    /// Consume a numeric literal.
    fn expect_number(&mut self, context: &'static str) -> Result<(f64, usize), A2lError> {
        let tok = self.bump_or(context)?;
        match &tok.kind {
            TokenKind::Number(value) => Ok((*value, tok.line)),
            _ => Err(A2lError::UnexpectedToken {
                expected: context,
                got: render(tok),
                line: tok.line,
            }),
        }
    }

    /// Consume a numeric literal that must be a `u32` ECU address.
    fn expect_address(&mut self, context: &'static str) -> Result<u32, A2lError> {
        let (value, line) = self.expect_number(context)?;
        if !(0.0..=f64::from(u32::MAX)).contains(&value) {
            return Err(A2lError::InvalidAddress { value, line });
        }
        // Range-checked above: the value fits u32 exactly.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        Ok(value as u32)
    }

    /// Skip tokens until after the matching `/end` of a `/begin` block
    /// already consumed by the caller. Tracks nesting across *all* block
    /// keywords, tolerates braces and punctuation, and enforces
    /// [`MAX_DEPTH`].
    fn skip_balanced(
        &mut self,
        block: &str,
        begin_line: usize,
        depth: usize,
    ) -> Result<(), A2lError> {
        if depth > MAX_DEPTH {
            let line = self.peek().map_or(0, |t| t.line);
            return Err(A2lError::DepthLimitExceeded { line });
        }
        loop {
            let Some(tok) = self.bump() else {
                return Err(A2lError::UnterminatedBlock {
                    block: block.to_owned(),
                    line: begin_line,
                });
            };
            match &tok.kind {
                TokenKind::Slash(word) if word == "begin" => {
                    // Consume the block keyword too.
                    self.bump_or("block keyword after /begin")?;
                    self.skip_balanced(block, begin_line, depth + 1)?;
                }
                TokenKind::Slash(word) if word == "end" => {
                    self.bump_or("block keyword after /end")?;
                    return Ok(());
                }
                _ => {}
            }
        }
    }

    /// Top level: `/begin PROJECT name "desc" … /end PROJECT`.
    fn parse_project(&mut self) -> Result<A2lProject, A2lError> {
        // Advance to the first /begin (tolerating leading junk/comments —
        // comments never produce tokens).
        while let Some(tok) = self.peek() {
            if matches!(tok.kind, TokenKind::Slash(ref w) if w == "begin") {
                break;
            }
            self.pos += 1;
        }
        if self.at_end() {
            return Err(A2lError::MissingProject);
        }
        let (block, begin_line) = self.expect_begin()?;
        if block != "PROJECT" {
            return Err(A2lError::UnexpectedToken {
                expected: "/begin PROJECT",
                got: block.to_owned(),
                line: begin_line,
            });
        }
        let project = self.expect_ident("PROJECT name")?.to_owned();
        // Long identifier is optional in practice.
        if let Some(Token {
            kind: TokenKind::Str(_),
            ..
        }) = self.peek()
        {
            self.bump();
        }

        let mut modules = Vec::new();
        loop {
            if self.at_end() {
                return Err(A2lError::UnterminatedBlock {
                    block: "PROJECT".to_owned(),
                    line: begin_line,
                });
            }
            let Some(tok) = self.peek() else {
                unreachable!("at_end() guards peek()");
            };
            let TokenKind::Slash(word) = &tok.kind else {
                self.pos += 1;
                continue;
            };
            if word == "end" {
                self.pos += 1;
                let (closed, line) = self.end_name()?;
                if closed == "PROJECT" {
                    return Ok(A2lProject { project, modules });
                }
                return Err(A2lError::BlockMismatch {
                    opened: "PROJECT".to_owned(),
                    closed: closed.to_owned(),
                    line,
                });
            }
            if word != "begin" {
                self.pos += 1;
                continue;
            }
            self.pos += 1; // /begin
            let (block, line) = self.begin_name()?;
            match block {
                "MODULE" => modules.push(self.parse_module()?),
                _ => self.skip_balanced(block, line, 1)?,
            }
        }
    }

    /// The block keyword after a `/begin` whose slash token was already
    /// consumed.
    fn begin_name(&mut self) -> Result<(&'t str, usize), A2lError> {
        let tok = self.bump_or("block keyword after /begin")?;
        match &tok.kind {
            TokenKind::Ident(name) => Ok((name.as_str(), tok.line)),
            _ => Err(A2lError::UnexpectedToken {
                expected: "block keyword after /begin",
                got: render(tok),
                line: tok.line,
            }),
        }
    }

    /// `/begin MODULE name "desc" … /end MODULE`.
    fn parse_module(&mut self) -> Result<A2lModule, A2lError> {
        let name = self.expect_ident("MODULE name")?.to_owned();
        if let Some(Token {
            kind: TokenKind::Str(_),
            ..
        }) = self.peek()
        {
            self.bump();
        }
        let mut module = A2lModule {
            name,
            ..A2lModule::default()
        };

        loop {
            let Some(tok) = self.peek() else {
                return Err(A2lError::UnterminatedBlock {
                    block: "MODULE".to_owned(),
                    line: 0,
                });
            };
            let TokenKind::Slash(word) = &tok.kind else {
                self.pos += 1;
                continue;
            };
            if word == "end" {
                self.pos += 1;
                let (closed, line) = self.end_name()?;
                if closed == "MODULE" {
                    return Ok(module);
                }
                return Err(A2lError::BlockMismatch {
                    opened: "MODULE".to_owned(),
                    closed: closed.to_owned(),
                    line,
                });
            }
            if word != "begin" {
                self.pos += 1;
                continue;
            }
            self.pos += 1;
            let (block, line) = self.begin_name()?;
            match block {
                "CHARACTERISTIC" => module.characteristics.push(self.parse_characteristic()?),
                "MEASUREMENT" => module.measurements.push(self.parse_measurement()?),
                "RECORD_LAYOUT" => module.record_layouts.push(self.parse_record_layout()?),
                "COMPU_METHOD" => module.compu_methods.push(self.parse_compu_method()?),
                _ => self.skip_balanced(block, line, 1)?,
            }
        }
    }

    /// `/begin CHARACTERISTIC name "desc" TYPE ADDRESS DEPOSIT MAX_DIFF
    /// CONVERSION LOWER UPPER … /end CHARACTERISTIC`.
    fn parse_characteristic(&mut self) -> Result<Characteristic, A2lError> {
        let name = self.expect_ident("CHARACTERISTIC name")?.to_owned();
        // Description string (quoted per the grammar).
        if let Some(tok) = self.peek() {
            if matches!(tok.kind, TokenKind::Str(_)) {
                self.bump();
            }
        }
        let type_tok_line;
        let type_text = {
            let tok = self.bump_or("CHARACTERISTIC type")?;
            type_tok_line = tok.line;
            match &tok.kind {
                TokenKind::Ident(text) => text.clone(),
                _ => {
                    return Err(A2lError::UnexpectedToken {
                        expected: "CHARACTERISTIC type",
                        got: render(tok),
                        line: tok.line,
                    })
                }
            }
        };
        let r#type = CharType::from_keyword(&type_text).ok_or(A2lError::InvalidCharType {
            text: type_text,
            line: type_tok_line,
        })?;
        let address = self.expect_address("CHARACTERISTIC address")?;
        let deposit = self.expect_ident("RECORD_LAYOUT reference")?.to_owned();
        let (max_diff, _) = self.expect_number("MAX_DIFF")?;
        let conversion = self.expect_ident("COMPU_METHOD reference")?.to_owned();
        let (lower_limit, _) = self.expect_number("LOWER_LIMIT")?;
        let (upper_limit, _) = self.expect_number("UPPER_LIMIT")?;

        self.skip_body("CHARACTERISTIC")?;
        Ok(Characteristic {
            name,
            r#type,
            address,
            deposit,
            max_diff,
            conversion,
            lower_limit,
            upper_limit,
        })
    }

    /// `/begin MEASUREMENT name "desc" DATATYPE CONVERSION RESOLUTION
    /// ACCURACY LOWER UPPER [ECU_ADDRESS n] … /end MEASUREMENT`.
    fn parse_measurement(&mut self) -> Result<Measurement, A2lError> {
        let name = self.expect_ident("MEASUREMENT name")?.to_owned();
        if let Some(tok) = self.peek() {
            if matches!(tok.kind, TokenKind::Str(_)) {
                self.bump();
            }
        }
        let datatype = self.expect_datatype()?;
        let conversion = self.expect_ident("COMPU_METHOD reference")?.to_owned();
        // RESOLUTION, ACCURACY, LOWER_LIMIT, UPPER_LIMIT (required by the
        // grammar, unused by the model).
        self.expect_number("RESOLUTION")?;
        self.expect_number("ACCURACY")?;
        self.expect_number("LOWER_LIMIT")?;
        self.expect_number("UPPER_LIMIT")?;

        // Body: watch for ECU_ADDRESS; skip the rest.
        let mut address = 0u32;
        loop {
            let Some(tok) = self.peek() else {
                return Err(A2lError::UnterminatedBlock {
                    block: "MEASUREMENT".to_owned(),
                    line: 0,
                });
            };
            match &tok.kind {
                TokenKind::Slash(word) if word == "end" => {
                    self.pos += 1;
                    let (closed, line) = self.end_name()?;
                    if closed == "MEASUREMENT" {
                        return Ok(Measurement {
                            name,
                            datatype,
                            address,
                            conversion,
                        });
                    }
                    return Err(A2lError::BlockMismatch {
                        opened: "MEASUREMENT".to_owned(),
                        closed: closed.to_owned(),
                        line,
                    });
                }
                TokenKind::Slash(word) if word == "begin" => {
                    self.pos += 1;
                    self.begin_name()?;
                    self.skip_balanced("MEASUREMENT", tok.line, 1)?;
                }
                TokenKind::Ident(keyword) if keyword == "ECU_ADDRESS" => {
                    self.pos += 1;
                    if self
                        .peek()
                        .is_some_and(|t| matches!(t.kind, TokenKind::Number(_)))
                    {
                        address = self.expect_address("ECU_ADDRESS value")?;
                    }
                }
                _ => self.pos += 1,
            }
        }
    }

    /// `/begin RECORD_LAYOUT name ["desc"] [FNC_VALUES …] … /end
    /// RECORD_LAYOUT`.
    fn parse_record_layout(&mut self) -> Result<RecordLayout, A2lError> {
        let name = self.expect_ident("RECORD_LAYOUT name")?.to_owned();
        if let Some(tok) = self.peek() {
            if matches!(tok.kind, TokenKind::Str(_)) {
                self.bump();
            }
        }
        let mut fnc_values = None;
        loop {
            let Some(tok) = self.peek() else {
                return Err(A2lError::UnterminatedBlock {
                    block: "RECORD_LAYOUT".to_owned(),
                    line: 0,
                });
            };
            match &tok.kind {
                TokenKind::Slash(word) if word == "end" => {
                    self.pos += 1;
                    let (closed, line) = self.end_name()?;
                    if closed == "RECORD_LAYOUT" {
                        return Ok(RecordLayout { name, fnc_values });
                    }
                    return Err(A2lError::BlockMismatch {
                        opened: "RECORD_LAYOUT".to_owned(),
                        closed: closed.to_owned(),
                        line,
                    });
                }
                TokenKind::Slash(word) if word == "begin" => {
                    self.pos += 1;
                    self.begin_name()?;
                    self.skip_balanced("RECORD_LAYOUT", tok.line, 1)?;
                }
                TokenKind::Ident(keyword) if keyword == "FNC_VALUES" => {
                    self.pos += 1;
                    let (position, _) = self.expect_number("FNC_VALUES POSITION")?;
                    let dt_line;
                    let datatype = {
                        let tok = self.bump_or("FNC_VALUES DATATYPE")?;
                        dt_line = tok.line;
                        match &tok.kind {
                            TokenKind::Ident(text) => {
                                DataType::from_keyword(text).ok_or(A2lError::InvalidDatatype {
                                    text: text.clone(),
                                    line: dt_line,
                                })?
                            }
                            _ => {
                                return Err(A2lError::UnexpectedToken {
                                    expected: "FNC_VALUES DATATYPE",
                                    got: render(tok),
                                    line: tok.line,
                                })
                            }
                        }
                    };
                    fnc_values = Some(FncValues {
                        // Clamped to 0..=255 directly above: no
                        // truncation, no sign loss.
                        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                        position: position.clamp(0.0, 255.0) as u8,
                        datatype,
                    });
                }
                _ => self.pos += 1,
            }
        }
    }

    /// `/begin COMPU_METHOD name "desc" TYPE "FORMAT" "UNIT" [COEFFS … |
    /// COEFFS_LINEAR a b | COMPU_TAB_REF name] … /end COMPU_METHOD`.
    fn parse_compu_method(&mut self) -> Result<CompuMethod, A2lError> {
        let name = self.expect_ident("COMPU_METHOD name")?.to_owned();
        if let Some(tok) = self.peek() {
            if matches!(tok.kind, TokenKind::Str(_)) {
                self.bump();
            }
        }
        let ct_line;
        let conversion_type = {
            let tok = self.bump_or("COMPU_METHOD conversion type")?;
            ct_line = tok.line;
            match &tok.kind {
                TokenKind::Ident(text) => {
                    ConversionType::from_keyword(text).ok_or(A2lError::InvalidConversionType {
                        text: text.clone(),
                        line: ct_line,
                    })?
                }
                _ => {
                    return Err(A2lError::UnexpectedToken {
                        expected: "COMPU_METHOD conversion type",
                        got: render(tok),
                        line: tok.line,
                    })
                }
            }
        };
        // FORMAT ("%.3f") and UNIT ("kg/h") — quoted strings per the
        // grammar; either may be omitted by sloppy generators, in which
        // case the following keyword is left for the body loop.
        if let Some(tok) = self.peek() {
            if matches!(tok.kind, TokenKind::Str(_)) {
                self.bump();
            }
        }
        let unit = match self.peek().map(|t| &t.kind) {
            Some(TokenKind::Str(text)) => {
                let unit = text.clone();
                self.pos += 1;
                unit
            }
            _ => String::new(),
        };

        let mut coeffs = Coeffs::None;
        let mut tab_ref = None;
        loop {
            let Some(tok) = self.peek() else {
                return Err(A2lError::UnterminatedBlock {
                    block: "COMPU_METHOD".to_owned(),
                    line: 0,
                });
            };
            match &tok.kind {
                TokenKind::Slash(word) if word == "end" => {
                    self.pos += 1;
                    let (closed, line) = self.end_name()?;
                    if closed == "COMPU_METHOD" {
                        return Ok(CompuMethod {
                            name,
                            conversion_type,
                            coeffs,
                            tab_ref,
                            unit,
                        });
                    }
                    return Err(A2lError::BlockMismatch {
                        opened: "COMPU_METHOD".to_owned(),
                        closed: closed.to_owned(),
                        line,
                    });
                }
                TokenKind::Slash(word) if word == "begin" => {
                    self.pos += 1;
                    self.begin_name()?;
                    self.skip_balanced("COMPU_METHOD", tok.line, 1)?;
                }
                TokenKind::Ident(keyword) if keyword == "COEFFS" => {
                    self.pos += 1;
                    let mut c = [0.0f64; 6];
                    for slot in &mut c {
                        (*slot, _) = self.expect_number("COEFFS coefficient")?;
                    }
                    coeffs = Coeffs::RatFunc(c);
                }
                TokenKind::Ident(keyword) if keyword == "COEFFS_LINEAR" => {
                    self.pos += 1;
                    let mut c = [0.0f64; 2];
                    for slot in &mut c {
                        (*slot, _) = self.expect_number("COEFFS_LINEAR coefficient")?;
                    }
                    coeffs = Coeffs::Linear(c);
                }
                TokenKind::Ident(keyword) if keyword == "COMPU_TAB_REF" => {
                    self.pos += 1;
                    tab_ref = Some(self.expect_ident("COMPU_TAB_REF name")?.to_owned());
                }
                _ => self.pos += 1,
            }
        }
    }

    /// Consume a DATATYPE keyword.
    fn expect_datatype(&mut self) -> Result<DataType, A2lError> {
        let tok = self.bump_or("DATATYPE keyword")?;
        match &tok.kind {
            TokenKind::Ident(text) => {
                DataType::from_keyword(text).ok_or(A2lError::InvalidDatatype {
                    text: text.clone(),
                    line: tok.line,
                })
            }
            _ => Err(A2lError::UnexpectedToken {
                expected: "DATATYPE keyword",
                got: render(tok),
                line: tok.line,
            }),
        }
    }

    /// Skip a known block's optional body up to and including its `/end`.
    fn skip_body(&mut self, block: &'static str) -> Result<(), A2lError> {
        loop {
            let Some(tok) = self.peek() else {
                return Err(A2lError::UnterminatedBlock {
                    block: block.to_owned(),
                    line: 0,
                });
            };
            match &tok.kind {
                TokenKind::Slash(word) if word == "end" => {
                    self.pos += 1;
                    let (closed, line) = self.end_name()?;
                    if closed == block {
                        return Ok(());
                    }
                    return Err(A2lError::BlockMismatch {
                        opened: block.to_owned(),
                        closed: closed.to_owned(),
                        line,
                    });
                }
                TokenKind::Slash(word) if word == "begin" => {
                    self.pos += 1;
                    self.begin_name()?;
                    self.skip_balanced(block, tok.line, 1)?;
                }
                _ => self.pos += 1,
            }
        }
    }
}

/// Render a token for error messages.
fn render(token: &Token) -> String {
    match &token.kind {
        TokenKind::Ident(text) | TokenKind::Slash(text) | TokenKind::Str(text) => text.clone(),
        TokenKind::Number(value) => format!("{value}"),
        TokenKind::Punct(c) => c.to_string(),
    }
}
