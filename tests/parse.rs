//! Parse a known-good A2L file (embedded realistic sample): block
//! structure, positional parameters, optional-keyword capture, lookups,
//! and skip-tolerance over vendor blocks.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use a2l_parse::{A2lProject, CharType, Coeffs, ConversionType, DataType, LinearFn};

const SAMPLE: &str = include_str!("files/sample.a2l");

#[test]
fn sample_parses_with_project_and_modules() {
    let project = A2lProject::parse(SAMPLE).unwrap();
    assert_eq!(project.project, "demo_ecu");
    assert_eq!(project.modules.len(), 2);
    assert_eq!(project.modules[0].name, "engine");
    assert_eq!(project.modules[1].name, "gearbox");
    assert_eq!(project.module_by_name("gearbox").unwrap().name, "gearbox");
    assert!(project.module_by_name("missing").is_none());
}

#[test]
fn sample_characteristics_full_fidelity() {
    let project = A2lProject::parse(SAMPLE).unwrap();
    let module = project.module_by_name("engine").unwrap();

    assert_eq!(module.characteristics.len(), 4);

    let load = module.characteristic_by_name("eng_load").unwrap();
    assert_eq!(load.r#type, CharType::Value);
    assert_eq!(load.address, 0x720000);
    assert_eq!(load.deposit, "RL_VALUE");
    assert_eq!(load.max_diff, 0.0);
    assert_eq!(load.conversion, "rack_lin");
    assert_eq!(load.lower_limit, 0.0);
    assert_eq!(load.upper_limit, 100.0);

    let map = module.characteristic_by_name("inj_map").unwrap();
    assert_eq!(map.r#type, CharType::Map);
    assert_eq!(map.address, 0x721000);
    assert_eq!(map.deposit, "RL_MAP_UWORD");

    let curve = module.characteristic_by_name("boost_curve").unwrap();
    assert_eq!(curve.r#type, CharType::Curve);
    assert_eq!(curve.address, 0x723000);

    let ascii = module.characteristic_by_name("ecu_id").unwrap();
    assert_eq!(ascii.r#type, CharType::Ascii);

    assert!(module.characteristic_by_name("shift_pressure").is_none());
}

#[test]
fn sample_measurements_capture_ecu_address() {
    let project = A2lProject::parse(SAMPLE).unwrap();
    let module = project.module_by_name("engine").unwrap();

    assert_eq!(module.measurements.len(), 4);

    let speed = module.measurement_by_name("engine_speed").unwrap();
    assert_eq!(speed.datatype, DataType::Uword);
    assert_eq!(speed.address, 0x320001);
    assert_eq!(speed.conversion, "engine_speed_c");

    let temp = module.measurement_by_name("coolant_temp").unwrap();
    assert_eq!(temp.datatype, DataType::Sbyte);
    assert_eq!(temp.address, 0x320100);

    let gear = module.measurement_by_name("gear_pos").unwrap();
    assert_eq!(gear.datatype, DataType::Ubyte);

    // No ECU_ADDRESS keyword → address 0, still parsed.
    let virt = module.measurement_by_name("virtual_power").unwrap();
    assert_eq!(virt.address, 0);
    assert_eq!(virt.conversion, "rack_lin");
}

#[test]
fn sample_record_layouts() {
    let project = A2lProject::parse(SAMPLE).unwrap();
    let module = project.module_by_name("engine").unwrap();

    let rl = module.record_layout_by_name("RL_MAP_UWORD").unwrap();
    let fnc = rl.fnc_values.expect("FNC_VALUES captured");
    assert_eq!(fnc.position, 1);
    assert_eq!(fnc.datatype, DataType::Uword);
    assert_eq!(module.record_layouts.len(), 3);

    // Gearbox module has its own RL_VALUE with a different datatype —
    // modules do not leak into each other.
    let project = A2lProject::parse(SAMPLE).unwrap();
    let gearbox = project.module_by_name("gearbox").unwrap();
    assert_eq!(
        gearbox
            .record_layout_by_name("RL_VALUE")
            .unwrap()
            .fnc_values
            .unwrap()
            .datatype,
        DataType::Ubyte
    );
}

#[test]
fn sample_compu_methods_and_conversion_types() {
    let project = A2lProject::parse(SAMPLE).unwrap();
    let module = project.module_by_name("engine").unwrap();

    assert_eq!(module.compu_methods.len(), 5);

    let speed = module.compu_method_by_name("engine_speed_c").unwrap();
    assert_eq!(speed.conversion_type, ConversionType::RatFunc);
    assert_eq!(
        speed.coeffs,
        Coeffs::RatFunc([0.0, 1.0, 0.0, 0.0, 0.0, 1.0])
    );
    assert_eq!(speed.unit, "1/min");
    assert!(speed.tab_ref.is_none());

    let gear = module.compu_method_by_name("gear_tab").unwrap();
    assert_eq!(gear.conversion_type, ConversionType::Table);
    assert_eq!(gear.tab_ref.as_deref(), Some("gear_vtab"));

    let identity = module.compu_method_by_name("identity_raw").unwrap();
    assert_eq!(identity.conversion_type, ConversionType::Identity);

    let temp = module.compu_method_by_name("temp_lin").unwrap();
    assert_eq!(temp.conversion_type, ConversionType::Linear);
    assert_eq!(temp.coeffs, Coeffs::Linear([0.75, -48.0]));
}

#[test]
fn compu_method_linear_conversion() {
    let project = A2lProject::parse(SAMPLE).unwrap();
    let module = project.module_by_name("engine").unwrap();

    // RAT_FUNC (0 b c 0 0 1) reduces to f(x) = b·x + c.
    let rack = module.compu_method_by_name("rack_lin").unwrap();
    let lin = rack.to_linear().unwrap();
    assert_eq!(
        lin,
        LinearFn {
            slope: 0.00390625,
            intercept: 0.0
        }
    );
    assert_eq!(lin.apply(0.0), 0.0);
    assert_eq!(lin.apply(256.0), 1.0);
    assert_eq!(lin.apply(1024.0), 4.0);

    // LINEAR applies directly: f(x) = 0.75·x − 48.
    let temp = module.compu_method_by_name("temp_lin").unwrap();
    let lin = temp.to_linear().unwrap();
    assert_eq!(lin.apply(0.0), -48.0);
    assert_eq!(lin.apply(100.0), 27.0);

    // IDENTITY is f(x) = x.
    let identity = module.compu_method_by_name("identity_raw").unwrap();
    let lin = identity.to_linear().unwrap();
    assert_eq!(lin.apply(42.0), 42.0);

    // TABLE is not reducible.
    let gear = module.compu_method_by_name("gear_tab").unwrap();
    assert!(gear.to_linear().is_none());
}

#[test]
fn rational_genuinely_nonlinear_is_none() {
    // f(x) = (2x² + x + 1) / (x² + 1) — d ≠ 0: irreducible.
    let cm = a2l_parse::CompuMethod {
        name: "quad".into(),
        conversion_type: ConversionType::RatFunc,
        coeffs: Coeffs::RatFunc([2.0, 1.0, 1.0, 1.0, 0.0, 1.0]),
        tab_ref: None,
        unit: String::new(),
    };
    assert!(cm.to_linear().is_none());

    // x² numerator with constant denominator is still not linear.
    let cm2 = a2l_parse::CompuMethod {
        name: "quad_const_den".into(),
        conversion_type: ConversionType::RatFunc,
        coeffs: Coeffs::RatFunc([2.0, 1.0, 1.0, 0.0, 0.0, 1.0]),
        tab_ref: None,
        unit: String::new(),
    };
    assert!(cm2.to_linear().is_none());

    // Zero-denominator constant form (d = e = f = 0) is invalid → None.
    let cm3 = a2l_parse::CompuMethod {
        name: "bad_den".into(),
        conversion_type: ConversionType::RatFunc,
        coeffs: Coeffs::RatFunc([0.0, 1.0, 0.0, 0.0, 0.0, 0.0]),
        tab_ref: None,
        unit: String::new(),
    };
    assert!(cm3.to_linear().is_none());
}

#[test]
fn datatype_roundtrips_and_sizes() {
    let keywords = [
        "UBYTE",
        "SBYTE",
        "UWORD",
        "SWORD",
        "ULONG",
        "SLONG",
        "A_UINT64",
        "A_INT64",
        "FLOAT32_IEEE",
        "FLOAT64_IEEE",
    ];
    for keyword in keywords {
        let dt = DataType::from_keyword(keyword).unwrap();
        assert_eq!(dt.keyword(), keyword);
    }
    assert!(DataType::from_keyword("UBIT64").is_none());
    assert!(DataType::from_keyword("ubyte").is_none()); // keywords are case-sensitive

    // Sizes.
    assert_eq!(DataType::Ubyte.size_bytes(), 1);
    assert_eq!(DataType::Sword.size_bytes(), 2);
    assert_eq!(DataType::Ulong.size_bytes(), 4);
    assert_eq!(DataType::Float32.size_bytes(), 4);
    assert_eq!(DataType::Float64.size_bytes(), 8);
    assert_eq!(DataType::Uint64.size_bytes(), 8);
}

#[test]
fn char_type_and_conversion_type_roundtrips() {
    for keyword in ["VALUE", "CURVE", "MAP", "VAL_BLK", "ASCII"] {
        let ct = CharType::from_keyword(keyword).unwrap();
        assert_eq!(ct.keyword(), keyword);
    }
    assert!(CharType::from_keyword("MATRIX").is_none());

    assert_eq!(
        ConversionType::from_keyword("RAT_FUNC").unwrap().keyword(),
        "RAT_FUNC"
    );
    assert_eq!(
        ConversionType::from_keyword("IDENTICAL").unwrap(),
        ConversionType::Identity
    );
    assert_eq!(
        ConversionType::from_keyword("IDENTITY").unwrap().keyword(),
        "IDENTITY"
    );
    assert!(ConversionType::from_keyword("LOOKUP").is_none());
}

#[test]
fn lookups_miss_cleanly() {
    let project = A2lProject::parse(SAMPLE).unwrap();
    let module = project.module_by_name("engine").unwrap();
    assert!(module.characteristic_by_name("nope").is_none());
    assert!(module.measurement_by_name("nope").is_none());
    assert!(module.record_layout_by_name("nope").is_none());
    assert!(module.compu_method_by_name("nope").is_none());
}

#[test]
fn minimal_document() {
    let project = A2lProject::parse("/begin PROJECT p \"d\" /end PROJECT").unwrap();
    assert_eq!(project.project, "p");
    assert!(project.modules.is_empty());

    let project = A2lProject::parse(
        "/begin PROJECT p \"d\"\n /begin MODULE m \"d\" /end MODULE\n/end PROJECT",
    )
    .unwrap();
    assert_eq!(project.modules.len(), 1);
    assert_eq!(project.modules[0].characteristics.len(), 0);
}

#[test]
fn comments_do_not_confuse_the_scanner() {
    let src = r#"
        /* block comment with /begin PROJECT inside */
        // line comment with /end PROJECT inside
        /begin PROJECT p "desc with /begin MODULE in a string"
          /* another /* unterminated nesting attempt */
          /begin MODULE m "d" /end MODULE
        /end PROJECT
    "#;
    let project = A2lProject::parse(src).unwrap();
    assert_eq!(project.project, "p");
    assert_eq!(project.modules.len(), 1);
}

#[test]
fn empty_input_is_missing_project() {
    assert_eq!(
        A2lProject::parse(""),
        Err(a2l_parse::A2lError::MissingProject)
    );
    assert_eq!(
        A2lProject::parse("   \n\t // only a comment\n"),
        Err(a2l_parse::A2lError::MissingProject)
    );
}

#[test]
fn errors_carry_line_numbers() {
    // Unterminated string on line 3:
    let src = "/begin PROJECT p\n  \"d\"\n  \"oops\n/end PROJECT\n";
    let err = A2lProject::parse(src).unwrap_err();
    assert!(matches!(
        err,
        a2l_parse::A2lError::UnterminatedString { line: 3 }
    ));

    // Unterminated block: /begin MODULE never closed.
    let src = "/begin PROJECT p \"d\"\n/begin MODULE m \"d\"\n/end PROJECT\n";
    let err = A2lProject::parse(src).unwrap_err();
    match &err {
        a2l_parse::A2lError::BlockMismatch {
            opened,
            closed,
            line,
        } => {
            assert_eq!(opened, "MODULE");
            assert_eq!(closed, "PROJECT");
            assert_eq!(*line, 3);
        }
        other => panic!("wrong error: {other:?}"),
    }
    assert_eq!(err.line(), Some(3));

    // Unknown datatype keyword.
    let src = "/begin PROJECT p \"d\"\n\
               /begin MODULE m \"d\"\n\
               /begin MEASUREMENT x \"d\" UBIT64 c 1 0 0 1 /end MEASUREMENT\n\
               /end MODULE\n\
               /end PROJECT\n";
    let err = A2lProject::parse(src).unwrap_err();
    assert!(matches!(
        err,
        a2l_parse::A2lError::InvalidDatatype { ref text, line: 3 } if text == "UBIT64"
    ));

    // Unknown characteristic type.
    let src = "/begin PROJECT p \"d\"\n\
               /begin MODULE m \"d\"\n\
               /begin CHARACTERISTIC c \"d\" TENSOR 0x0 RL 0 cm 0 1 /end CHARACTERISTIC\n\
               /end MODULE\n\
               /end PROJECT\n";
    let err = A2lProject::parse(src).unwrap_err();
    assert!(matches!(
        err,
        a2l_parse::A2lError::InvalidCharType { ref text, .. } if text == "TENSOR"
    ));

    // Unknown conversion type.
    let src = "/begin PROJECT p \"d\"\n\
               /begin MODULE m \"d\"\n\
               /begin COMPU_METHOD c \"d\" MAGIC \"%.0f\" \"-\" /end COMPU_METHOD\n\
               /end MODULE\n\
               /end PROJECT\n";
    let err = A2lProject::parse(src).unwrap_err();
    assert!(matches!(
        err,
        a2l_parse::A2lError::InvalidConversionType { ref text, .. } if text == "MAGIC"
    ));

    // Address overflow beyond u32.
    let src = "/begin PROJECT p \"d\"\n\
               /begin MODULE m \"d\"\n\
               /begin CHARACTERISTIC c \"d\" VALUE 0x1FFFFFFFF RL 0 cm 0 1 /end CHARACTERISTIC\n\
               /end MODULE\n\
               /end PROJECT\n";
    let err = A2lProject::parse(src).unwrap_err();
    assert!(matches!(err, a2l_parse::A2lError::InvalidAddress { .. }));
}

#[test]
fn deep_nesting_is_bounded() {
    // 40 nested unknown blocks exceed the depth guard.
    let mut src = String::from("/begin PROJECT p \"d\"\n");
    for _ in 0..40 {
        src.push_str("/begin VENDOR_BLOCK \"x\"\n");
    }
    for _ in 0..40 {
        src.push_str("/end VENDOR_BLOCK\n");
    }
    src.push_str("/end PROJECT\n");
    let err = A2lProject::parse(&src).unwrap_err();
    assert!(matches!(
        err,
        a2l_parse::A2lError::DepthLimitExceeded { .. }
    ));
}

#[test]
fn braces_and_junk_are_skipped() {
    let src = "/begin PROJECT p \"d\"\n\
               /begin MODULE m \"d\"\n\
               { junk } = , : random tokens ! @ #\n\
               /begin MODULE m2 \"d\" /end MODULE\n\
               /end MODULE\n\
               /end PROJECT\n";
    let project = A2lProject::parse(src).unwrap();
    assert_eq!(project.modules.len(), 1);
    assert_eq!(project.modules[0].name, "m");
}
