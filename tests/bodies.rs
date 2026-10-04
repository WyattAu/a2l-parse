//! Body-loop discipline inside the typed blocks: wrong `/end` closes,
//! nested unknown sub-blocks, keyword bodies, and line-comment/junk
//! tolerance at the module level.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use a2l_parse::{A2lError, A2lProject, Coeffs, ConversionType};

fn parse(src: &str) -> Result<A2lProject, A2lError> {
    A2lProject::parse(src)
}

fn wrap(body: &str) -> String {
    format!("/begin PROJECT p \"d\"\n/begin MODULE m \"d\"\n{body}/end MODULE\n/end PROJECT\n")
}

#[test]
fn wrong_end_close_in_body_loops() {
    let cases = [
        (
            "/begin MEASUREMENT x \"d\" UWORD cm 0 0 0 100\n/end MODULE\n",
            "MEASUREMENT",
        ),
        ("/begin RECORD_LAYOUT rl\n/end MODULE\n", "RECORD_LAYOUT"),
        (
            "/begin COMPU_METHOD cm \"d\" LINEAR \"%.1f\" \"x\"\n/end MODULE\n",
            "COMPU_METHOD",
        ),
        (
            "/begin CHARACTERISTIC c \"d\" VALUE 0x10 RL 0 cm 0 1\n/end MODULE\n",
            "CHARACTERISTIC",
        ),
    ];
    for (body, opened) in cases {
        let err = parse(&wrap(body)).unwrap_err();
        assert!(
            matches!(&err, A2lError::BlockMismatch { opened: o, closed, .. }
                if o == opened && closed == "MODULE"),
            "{opened}: {err:?}"
        );
    }
}

#[test]
fn nested_unknown_blocks_inside_typed_bodies() {
    // Each typed body tolerates nested /begin blocks of unknown kinds.
    let src = wrap(
        "/begin MEASUREMENT x \"d\" UWORD cm 0 0 0 100\n\
         /begin VENDOR_TAG \"t\"\n /begin DEEPER 1 2 3 /end DEEPER\n /end VENDOR_TAG\n\
         ECU_ADDRESS 0x42\n\
         /end MEASUREMENT\n\
         /begin RECORD_LAYOUT rl\n\
         /begin VENDOR_TAG \"t\" /end VENDOR_TAG\n\
         FNC_VALUES 2 UWORD COLUMN_DIR DIRECT\n\
         /end RECORD_LAYOUT\n\
         /begin COMPU_METHOD cm \"d\" LINEAR \"%.1f\" \"x\"\n\
         /begin VENDOR_TAG \"t\" /end VENDOR_TAG\n\
         COEFFS_LINEAR 2 3\n\
         /end COMPU_METHOD\n",
    );
    let project = parse(&src).unwrap();
    let module = &project.modules[0];
    assert_eq!(module.measurements[0].address, 0x42);
    let rl = &module.record_layouts[0];
    assert_eq!(rl.fnc_values.unwrap().position, 2);
    let cm = &module.compu_methods[0];
    assert_eq!(cm.coeffs, Coeffs::Linear([2.0, 3.0]));
    assert_eq!(cm.to_linear().unwrap().apply(1.0), 5.0);
}

#[test]
fn stray_slash_keywords_at_module_level_are_skipped() {
    let src = wrap("/keyword /another 42\n/begin MODULE inner \"d\" /end MODULE\n");
    let project = parse(&src).unwrap();
    assert_eq!(project.modules[0].name, "m");
}

#[test]
fn begin_with_non_ident_block_name_errors() {
    // /begin followed by a string is a typed error from begin_name.
    let src = wrap("/begin \"nameless\" \"d\"\n/end \"nameless\"\n");
    let err = parse(&src).unwrap_err();
    assert!(matches!(
        err,
        A2lError::UnexpectedToken {
            expected: "block keyword after /begin",
            ..
        }
    ));
}

#[test]
fn record_layout_takes_quoted_description() {
    let src = wrap("/begin RECORD_LAYOUT rl \"deposit description\"\nFNC_VALUES 1 UBYTE DIRECT\n/end RECORD_LAYOUT\n");
    let project = parse(&src).unwrap();
    let rl = &project.modules[0].record_layouts[0];
    assert_eq!(rl.name, "rl");
    assert_eq!(rl.fnc_values.unwrap().datatype, a2l_parse::DataType::Ubyte);
}

#[test]
fn fnc_values_datatype_must_be_ident() {
    let src = wrap("/begin RECORD_LAYOUT rl\nFNC_VALUES 1 5 COLUMN_DIR\n/end RECORD_LAYOUT\n");
    let err = parse(&src).unwrap_err();
    assert!(matches!(
        err,
        A2lError::UnexpectedToken { expected: "FNC_VALUES DATATYPE", got, .. } if got == "5"
    ));
}

#[test]
fn compu_method_without_coeffs_is_not_linear() {
    // LINEAR without COEFFS_LINEAR: unit/format stay empty, coeffs None,
    // no linear reduction.
    let src = wrap("/begin COMPU_METHOD cm \"d\" LINEAR\n/end COMPU_METHOD\n");
    let project = parse(&src).unwrap();
    let cm = &project.modules[0].compu_methods[0];
    assert_eq!(cm.conversion_type, ConversionType::Linear);
    assert_eq!(cm.coeffs, Coeffs::None);
    assert_eq!(cm.unit, "");
    assert!(cm.to_linear().is_none());

    // Same for RAT_FUNC without COEFFS.
    let src = wrap("/begin COMPU_METHOD cm2 \"d\" RAT_FUNC \"%.0f\" \"-\"\n/end COMPU_METHOD\n");
    let project = parse(&src).unwrap();
    let cm = &project.modules[0].compu_methods[0];
    assert_eq!(cm.coeffs, Coeffs::None);
    assert!(cm.to_linear().is_none());
}

#[test]
fn line_comments_and_blank_lines_anywhere() {
    let src = "// leading\n\
               /begin PROJECT p \"d\" // trailing on begin\n\
               // between\n\
               /begin MODULE m \"d\"\n\
               // inside module\n\
               /begin MEASUREMENT x \"d\" UWORD cm 0 0 0 100\n\
               // inside measurement\n\
               ECU_ADDRESS 0x7 // after address\n\
               /end MEASUREMENT\n\
               /end MODULE\n\
               /end PROJECT // done\n";
    let project = parse(src).unwrap();
    assert_eq!(project.modules[0].measurements[0].address, 0x7);
}

#[test]
fn description_strings_may_span_lines() {
    let src = "/begin PROJECT p \"multi\nline\nidentifier\"\n/end PROJECT\n";
    let project = parse(src).unwrap();
    assert_eq!(project.project, "p");
}
