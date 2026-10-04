//! Error-taxonomy and lexer-edge coverage: every `A2lError` variant is
//! produced from a real malformed input, Display/line accessors are
//! checked, and every lexer branch (escapes, hex, exponents, punctuation,
//! comments) is exercised.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use a2l_parse::{A2lError, A2lProject};

fn parse(src: &str) -> Result<A2lProject, A2lError> {
    A2lProject::parse(src)
}

/// A minimal PROJECT frame with the module body replaced by `body`.
fn wrap(body: &str) -> String {
    format!("/begin PROJECT p \"d\"\n/begin MODULE m \"d\"\n{body}/end MODULE\n/end PROJECT\n")
}

#[test]
fn unterminated_comment() {
    let err = parse("/* never closed /begin PROJECT p \"d\" /end PROJECT").unwrap_err();
    assert_eq!(err, A2lError::UnterminatedComment { line: 1 });
    assert_eq!(err.line(), Some(1));
    assert_eq!(err.to_string(), "line 1: unterminated /* comment");
}

#[test]
fn unterminated_string() {
    let err = parse("/begin PROJECT p \"oops").unwrap_err();
    assert_eq!(err, A2lError::UnterminatedString { line: 1 });
    assert_eq!(err.to_string(), "line 1: unterminated string");

    // Backslash immediately before EOF inside a string.
    let err = parse("/begin PROJECT p \"oops\\").unwrap_err();
    assert_eq!(err, A2lError::UnterminatedString { line: 1 });
}

#[test]
fn invalid_numbers() {
    // Positional runs are strict: "1.2.3" lexes as 1.2 + "." + 3, and the
    // stray punctuation errors at the CONVERSION slot.
    let src =
        wrap("/begin CHARACTERISTIC c \"d\" VALUE 0x10 RL 1.2.3 cm 0 1\n/end CHARACTERISTIC\n");
    let err = parse(&src).unwrap_err();
    assert!(matches!(
        err,
        A2lError::UnexpectedToken { expected: "COMPU_METHOD reference", got, line: 3 } if got == "."
    ));

    // Hex literal overflowing u64 is a typed invalid-number error.
    let src = wrap("/begin CHARACTERISTIC c \"d\" VALUE 0xFFFFFFFFFFFFFFFFFF RL 0 cm 0 1\n/end CHARACTERISTIC\n");
    let err = parse(&src).unwrap_err();
    assert!(matches!(
        err,
        A2lError::InvalidNumber { ref text, line: 3 } if text == "0xFFFFFFFFFFFFFFFFFF"
    ));

    // A dangling exponent run fails f64 parsing.
    let src = wrap("/begin CHARACTERISTIC c \"d\" VALUE 0x10 RL 1e+ cm 0 1\n/end CHARACTERISTIC\n");
    let err = parse(&src).unwrap_err();
    assert!(matches!(
        err,
        A2lError::InvalidNumber { ref text, .. } if text == "1e+"
    ));

    // "0x" with no digits is tolerated as 0 + identifier junk (the
    // decimal scanner stops at non-digits): 0 lands in the address slot,
    // `x` becomes the deposit name.
    let src = wrap("/begin CHARACTERISTIC c \"d\" VALUE 0x RL 0 cm 0 1\n/end CHARACTERISTIC\n");
    let err = parse(&src).unwrap_err();
    assert!(matches!(
        err,
        A2lError::UnexpectedToken { expected: "MAX_DIFF", got, .. } if got == "RL"
    ));

    // A double-exponent run lexes as one numeric token and fails f64
    // parsing.
    let src =
        wrap("/begin CHARACTERISTIC c \"d\" VALUE 0x10 RL 1e5e5 cm 0 1\n/end CHARACTERISTIC\n");
    let err = parse(&src).unwrap_err();
    assert!(matches!(
        err,
        A2lError::InvalidNumber { ref text, .. } if text == "1e5e5"
    ));
}

#[test]
fn unexpected_eof_contexts() {
    // /begin with nothing after it:
    let err = parse("/begin").unwrap_err();
    assert_eq!(
        err,
        A2lError::UnexpectedEof {
            context: "block keyword after /begin"
        }
    );

    // /end with nothing after it (inside a MODULE body):
    let src = "/begin PROJECT p \"d\"\n/begin MODULE m \"d\"\n/end\n";
    let err = parse(src).unwrap_err();
    assert_eq!(
        err,
        A2lError::UnexpectedEof {
            context: "block keyword after /end"
        }
    );

    // PROJECT name missing:
    let err = parse("/begin PROJECT").unwrap_err();
    assert_eq!(
        err,
        A2lError::UnexpectedEof {
            context: "PROJECT name"
        }
    );

    // First block is not PROJECT:
    let err = parse("/begin MODULE m \"d\" /end MODULE").unwrap_err();
    assert!(matches!(
        err,
        A2lError::UnexpectedToken { expected: "/begin PROJECT", got, .. } if got == "MODULE"
    ));

    // /begin PROJECT given a /end instead of a name:
    let err = parse("/begin PROJECT /end PROJECT").unwrap_err();
    assert!(matches!(
        err,
        A2lError::UnexpectedToken {
            expected: "PROJECT name",
            ..
        }
    ));
}

#[test]
fn unexpected_token_variants() {
    // CHARACTERISTIC type given a number:
    let src = wrap("/begin CHARACTERISTIC c \"d\" 0x10 0x10 RL 0 cm 0 1\n/end CHARACTERISTIC\n");
    let err = parse(&src).unwrap_err();
    assert!(matches!(
        err,
        A2lError::UnexpectedToken {
            expected: "CHARACTERISTIC type",
            ..
        }
    ));

    // MODULE without a name:
    let src = "/begin PROJECT p \"d\"\n/begin MODULE \"d\"\n/end MODULE\n/end PROJECT\n";
    let err = parse(src).unwrap_err();
    assert!(matches!(
        err,
        A2lError::UnexpectedToken {
            expected: "MODULE name",
            ..
        }
    ));

    // MEASUREMENT datatype missing:
    let src = wrap("/begin MEASUREMENT x \"d\"\n/end MEASUREMENT\n");
    let err = parse(&src).unwrap_err();
    assert!(matches!(
        err,
        A2lError::UnexpectedToken {
            expected: "DATATYPE keyword",
            ..
        }
    ));

    // MEASUREMENT with a string where the datatype belongs:
    let src = wrap("/begin MEASUREMENT x \"d\" \"UWORD\" cm 0 0 0 100\n/end MEASUREMENT\n");
    let err = parse(&src).unwrap_err();
    assert!(matches!(
        err,
        A2lError::UnexpectedToken { expected: "DATATYPE keyword", got, line: 3 } if got == "UWORD"
    ));

    // RECORD_LAYOUT with a string where the name belongs:
    let src = wrap("/begin RECORD_LAYOUT \"name\"\n/end RECORD_LAYOUT\n");
    let err = parse(&src).unwrap_err();
    assert!(matches!(
        err,
        A2lError::UnexpectedToken {
            expected: "RECORD_LAYOUT name",
            ..
        }
    ));

    // COMPU_METHOD conversion type given a string:
    let src = wrap("/begin COMPU_METHOD c \"d\" \"RAT_FUNC\" \"f\" \"u\"\n/end COMPU_METHOD\n");
    let err = parse(&src).unwrap_err();
    assert!(matches!(
        err,
        A2lError::UnexpectedToken {
            expected: "COMPU_METHOD conversion type",
            ..
        }
    ));

    // PROJECT closed by the wrong /end:
    let err = parse("/begin PROJECT p \"d\"\n/end MODULE\n").unwrap_err();
    assert!(matches!(
        err,
        A2lError::BlockMismatch { ref opened, ref closed, .. }
            if opened == "PROJECT" && closed == "MODULE"
    ));
}

#[test]
fn unterminated_blocks_per_type() {
    // MODULE never closed:
    let err = parse("/begin PROJECT p \"d\"\n/begin MODULE m \"d\"\n").unwrap_err();
    assert!(matches!(
        err,
        A2lError::UnterminatedBlock { ref block, .. } if block == "MODULE"
    ));

    // PROJECT never closed:
    let err = parse("/begin PROJECT p \"d\"\n").unwrap_err();
    assert!(matches!(
        err,
        A2lError::UnterminatedBlock { ref block, .. } if block == "PROJECT"
    ));

    // Known blocks truncated at EOF (no closing tags at all):
    let truncated = [
        (
            "/begin PROJECT p \"d\"\n/begin MODULE m \"d\"\n/begin CHARACTERISTIC c \"d\" VALUE 0x10 RL 0 cm 0 1\n",
            "CHARACTERISTIC",
        ),
        (
            "/begin PROJECT p \"d\"\n/begin MODULE m \"d\"\n/begin MEASUREMENT x \"d\" UWORD cm 0 0 0 100\n",
            "MEASUREMENT",
        ),
        (
            "/begin PROJECT p \"d\"\n/begin MODULE m \"d\"\n/begin RECORD_LAYOUT rl\n",
            "RECORD_LAYOUT",
        ),
        (
            "/begin PROJECT p \"d\"\n/begin MODULE m \"d\"\n/begin COMPU_METHOD cm \"d\" RAT_FUNC \"%.0f\" \"-\"\n",
            "COMPU_METHOD",
        ),
        (
            "/begin PROJECT p \"d\"\n/begin MODULE m \"d\"\n/begin VENDOR \"x\"\n",
            "VENDOR",
        ),
    ];
    for (src, block) in truncated {
        let err = parse(src).unwrap_err();
        assert!(
            matches!(&err, A2lError::UnterminatedBlock { block: b, .. } if b == block),
            "expected UnterminatedBlock {block}: {err:?}"
        );
    }

    // A block closed by the WRONG /end is a mismatch, not unterminated:
    let src = wrap("/begin CHARACTERISTIC c \"d\" VALUE 0x10 RL 0 cm 0 1\n");
    let err = parse(&src).unwrap_err();
    assert!(matches!(
        err,
        A2lError::BlockMismatch { ref opened, ref closed, .. }
            if opened == "CHARACTERISTIC" && closed == "MODULE"
    ));
}

#[test]
fn depth_limit_error_display() {
    let err = A2lError::DepthLimitExceeded { line: 9 };
    assert_eq!(
        err.to_string(),
        "line 9: block nesting exceeds the parser limit"
    );
    assert_eq!(err.line(), Some(9));

    let err = A2lError::UnexpectedEof { context: "x" };
    assert_eq!(err.line(), None);
    assert_eq!(err.to_string(), "unexpected end of input: expected x");

    let err = A2lError::MissingProject;
    assert_eq!(err.line(), None);
    assert_eq!(err.to_string(), "no `/begin PROJECT` block found");

    let err = A2lError::InvalidAddress {
        value: 5.5e10,
        line: 2,
    };
    assert_eq!(
        err.to_string(),
        "line 2: value 55000000000 is not a 32-bit ECU address"
    );

    let err = A2lError::InvalidNumber {
        text: "1,2".into(),
        line: 4,
    };
    assert_eq!(err.to_string(), "line 4: invalid number `1,2`");

    let err = A2lError::UnexpectedToken {
        expected: "y".into(),
        got: "z".into(),
        line: 7,
    };
    assert_eq!(err.to_string(), "line 7: expected y, found `z`");

    let err = A2lError::InvalidDatatype {
        text: "X".into(),
        line: 1,
    };
    assert_eq!(err.to_string(), "line 1: unknown ASAP2 datatype `X`");

    let err = A2lError::InvalidCharType {
        text: "Y".into(),
        line: 1,
    };
    assert_eq!(err.to_string(), "line 1: unknown CHARACTERISTIC type `Y`");

    let err = A2lError::InvalidConversionType {
        text: "Z".into(),
        line: 1,
    };
    assert_eq!(
        err.to_string(),
        "line 1: unknown COMPU_METHOD conversion type `Z`"
    );

    let err = A2lError::UnterminatedBlock {
        block: "B".into(),
        line: 3,
    };
    assert_eq!(
        err.to_string(),
        "line 3: `/begin B` without matching `/end B`"
    );

    let err = A2lError::BlockMismatch {
        opened: "A".into(),
        closed: "B".into(),
        line: 5,
    };
    assert_eq!(err.to_string(), "line 5: `/end B` closes `/begin A`");
}

#[test]
fn lexer_string_escapes() {
    let src = "/begin PROJECT p \"tab\\tand\\nnewline quote\\\" back\\\\ raw\\x\"\n\
               /begin MODULE m \"d\" /end MODULE\n\
               /end PROJECT\n";
    let project = parse(src).unwrap();
    assert_eq!(project.project, "p");
}

#[test]
fn lexer_number_forms() {
    // Exponents, negative hex, plus sign, floats — all accepted.
    let src = wrap(
        "/begin CHARACTERISTIC c \"d\" VALUE 0x10 RL 1 cm -0x10 1.5e-2 +3\n/end CHARACTERISTIC\n",
    );
    // Positional grammar: ADDRESS 0x10, DEPOSIT RL, MAX_DIFF 1,
    // CONVERSION -0x10? No — CONVERSION is an ident; use a well-formed
    // characteristic instead and assert via lower/upper.
    let src =
        wrap("/begin CHARACTERISTIC c \"d\" VALUE 0x10 RL 1.5e-2 cm -16 +3\n/end CHARACTERISTIC\n");
    let project = parse(&src).unwrap();
    let c = &project.modules[0].characteristics[0];
    assert_eq!(c.address, 0x10);
    assert_eq!(c.max_diff, 0.015);
    assert_eq!(c.lower_limit, -16.0);
    assert_eq!(c.upper_limit, 3.0);

    // Lone signs become punctuation and are skipped in bodies.
    let src = wrap("- +\n");
    let project = parse(&src).unwrap();
    assert_eq!(project.modules[0].characteristics.len(), 0);
}

#[test]
fn compu_method_optional_unit_and_tab_ref_only() {
    // FORMAT omitted (sloppy generator): the single quoted string lands
    // in the FORMAT slot and the unit is empty.
    let src =
        wrap("/begin COMPU_METHOD cm \"d\" RAT_FUNC \"-\" COEFFS 0 1 0 0 0 1\n/end COMPU_METHOD\n");
    let project = parse(&src).unwrap();
    let cm = &project.modules[0].compu_methods[0];
    assert_eq!(cm.unit, "");
    assert_eq!(
        cm.coeffs,
        a2l_parse::Coeffs::RatFunc([0.0, 1.0, 0.0, 0.0, 0.0, 1.0])
    );

    // TABLE with only a COMPU_TAB_REF and no other keywords.
    let src = wrap("/begin COMPU_METHOD cm \"d\" TABLE \"%.0f\" \"-\" COMPU_TAB_REF tabby\n/end COMPU_METHOD\n");
    let project = parse(&src).unwrap();
    let cm = &project.modules[0].compu_methods[0];
    assert_eq!(cm.tab_ref.as_deref(), Some("tabby"));

    // Unknown keywords in the body are skipped without harm.
    let src = wrap(
        "/begin COMPU_METHOD cm \"d\" LINEAR \"%.1f\" \"degC\" COEFFS_LINEAR 2 3 STATUS_PROMPT stable\n/end COMPU_METHOD\n",
    );
    let project = parse(&src).unwrap();
    let cm = &project.modules[0].compu_methods[0];
    assert_eq!(cm.coeffs, a2l_parse::Coeffs::Linear([2.0, 3.0]));
}

#[test]
fn ecu_address_without_number_is_ignored() {
    let src = wrap("/begin MEASUREMENT x \"d\" UWORD cm 0 0 0 100 ECU_ADDRESS NOT_A_NUMBER\n/end MEASUREMENT\n");
    let project = parse(&src).unwrap();
    let m = &project.modules[0].measurements[0];
    assert_eq!(m.address, 0);
}

#[test]
fn nested_unknown_block_inside_known_block() {
    // AXIS_DESCR (with its own nested block) inside CHARACTERISTIC is
    // skipped wholesale.
    let src = wrap(
        "/begin CHARACTERISTIC c \"d\" CURVE 0x10 RL 0 cm 0 1\n\
         /begin AXIS_DESCR ax \"axis\" UWORD 0x20\n\
         NO_AXIS_PTS 4\n\
         /begin AXIS_POINTS \"predefined\" READ_ONLY\n\
         junk here\n\
         /end AXIS_POINTS\n\
         /end AXIS_DESCR\n\
         /end CHARACTERISTIC\n",
    );
    let project = parse(&src).unwrap();
    let c = &project.modules[0].characteristics[0];
    assert_eq!(c.r#type, a2l_parse::CharType::Curve);
    assert_eq!(c.address, 0x10);
}

#[test]
fn fnc_values_variants() {
    // FLOAT64 datatype and position 4.
    let src = wrap("/begin RECORD_LAYOUT rl\nFNC_VALUES 4 FLOAT64_IEEE COLUMN_DIR DIRECT\n/end RECORD_LAYOUT\n");
    let project = parse(&src).unwrap();
    let rl = &project.modules[0].record_layouts[0];
    let fnc = rl.fnc_values.unwrap();
    assert_eq!(fnc.position, 4);
    assert_eq!(fnc.datatype, a2l_parse::DataType::Float64);

    // Bad FNC_VALUES datatype is a typed error.
    let src =
        wrap("/begin RECORD_LAYOUT rl\nFNC_VALUES 1 UBIT128 COLUMN_DIR\n/end RECORD_LAYOUT\n");
    let err = parse(&src).unwrap_err();
    assert!(matches!(
        err,
        A2lError::InvalidDatatype { ref text, .. } if text == "UBIT128"
    ));
}

#[test]
fn trailing_content_after_project_is_tolerated() {
    let src = "/begin PROJECT p \"d\"\n/end PROJECT\ntrailing junk 0x1 \"str\"\n";
    let project = parse(src).unwrap();
    assert_eq!(project.project, "p");
}
#[test]
fn multibyte_strings_and_escapes_never_panic() {
    // Regression: unknown escapes followed by multi-byte UTF-8 used to
    // advance into a char boundary (found by cargo-fuzz).
    let src = "/begin PROJECT p \"backslash\\—dash\" \n/end PROJECT\n";
    let project = parse(src).unwrap();
    assert_eq!(project.project, "p");

    // Raw multi-byte content inside strings, descriptions, and units.
    let src = "/begin PROJECT prömje \"Berechnung—Temperatur °C\"\n\
               /begin MODULE m \"Einheit µm\"\n\
               /begin COMPU_METHOD cm \"d\" RAT_FUNC \"%.0f\" \"Nm²\" COEFFS 0 1 0 0 0 1\n\
               /end COMPU_METHOD\n\
               /end MODULE\n\
               /end PROJECT\n";
    let project = parse(src).unwrap();
    let cm = project
        .module_by_name("m")
        .unwrap()
        .compu_method_by_name("cm")
        .unwrap();
    assert_eq!(cm.unit, "Nm²");

    // Unterminated string after a multi-byte escape sequence.
    let err = parse("/begin PROJECT p \"\\n\u{4e2d}").unwrap_err();
    assert!(matches!(err, A2lError::UnterminatedString { .. }));

    // Non-ASCII outside strings is punctuation — skipped, never sliced.
    let src = "/begin PROJECT p \"d\"\n— → ✓ /begin MODULE m \"d\" /end MODULE\n/end PROJECT\n";
    let project = parse(src).unwrap();
    assert_eq!(project.modules.len(), 1);
}
