//! proptest: address parsing round-trips (hex and decimal encodings) and
//! document-generation fidelity. 500 cases per property.
//!
//! Test file: strategic unwraps are by design.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use a2l_parse::{A2lProject, CharType, DataType};
use proptest::prelude::*;

/// Render a u32 address the way real A2L generators do (hex), with a
/// plain-decimal variant.
fn render_address(addr: u32, hex: bool) -> String {
    if hex {
        format!("0x{addr:X}")
    } else {
        format!("{addr}")
    }
}

fn characteristic_doc(name: &str, char_type: &str, addr: &str, lower: f64, upper: f64) -> String {
    format!(
        "/begin PROJECT p \"d\"\n\
         /begin MODULE m \"d\"\n\
         /begin CHARACTERISTIC {name} \"desc\" {char_type} {addr} RL 0 cm {lower} {upper}\n\
         /end CHARACTERISTIC\n\
         /end MODULE\n\
         /end PROJECT\n"
    )
}

fn measurement_doc(name: &str, datatype: &str, addr: Option<u32>) -> String {
    let ecu = addr
        .map(|a| format!("ECU_ADDRESS {} ", render_address(a, true)))
        .unwrap_or_default();
    format!(
        "/begin PROJECT p \"d\"\n\
         /begin MODULE m \"d\"\n\
         /begin MEASUREMENT {name} \"desc\" {datatype} cm 0 0 0 100 {ecu}\n\
         /end MEASUREMENT\n\
         /end MODULE\n\
         /end PROJECT\n"
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// Any u32 address survives a hex encode→parse round-trip through a
    /// CHARACTERISTIC.
    #[test]
    fn characteristic_address_roundtrip(
        addr in any::<u32>(),
        char_type in prop::sample::select(vec!["VALUE", "CURVE", "MAP", "VAL_BLK", "ASCII"]),
        lower in -1e6f64..1e6,
        span in 0f64..1e6,
        hex in any::<bool>(),
    ) {
        let doc = characteristic_doc("kfile", char_type, &render_address(addr, hex), lower, lower + span);
        let project = A2lProject::parse(&doc).unwrap();
        prop_assert_eq!(project.project, "p");
        let module = &project.modules[0];
        let c = module.characteristic_by_name("kfile").unwrap();
        prop_assert_eq!(c.address, addr);
        prop_assert_eq!(c.r#type, CharType::from_keyword(char_type).unwrap());
        prop_assert_eq!(c.deposit.as_str(), "RL");
        prop_assert_eq!(c.conversion.as_str(), "cm");
        prop_assert_eq!(c.lower_limit, lower);
        prop_assert_eq!(c.upper_limit, lower + span);
    }

    /// Any u32 address survives a hex encode→parse round-trip through
    /// MEASUREMENT's ECU_ADDRESS keyword; absence decodes as 0.
    #[test]
    fn measurement_address_roundtrip(
        addr in any::<u32>(),
        datatype in prop::sample::select(vec![
            "UBYTE", "SBYTE", "UWORD", "SWORD", "ULONG", "SLONG",
            "A_UINT64", "A_INT64", "FLOAT32_IEEE", "FLOAT64_IEEE",
        ]),
        with_address in any::<bool>(),
    ) {
        let expected = with_address.then_some(addr);
        let doc = measurement_doc("msig", datatype, expected);
        let project = A2lProject::parse(&doc).unwrap();
        let m = project.modules[0].measurement_by_name("msig").unwrap();
        prop_assert_eq!(m.address, expected.unwrap_or(0));
        prop_assert_eq!(m.datatype, DataType::from_keyword(datatype).unwrap());
    }

    /// Every ASAP2 datatype round-trips through its keyword in a parsed
    /// document, and its model size is stable.
    #[test]
    fn datatype_document_roundtrip(
        datatype in prop::sample::select(vec![
            "UBYTE", "SBYTE", "UWORD", "SWORD", "ULONG", "SLONG",
            "A_UINT64", "A_INT64", "FLOAT32_IEEE", "FLOAT64_IEEE",
        ]),
        addr in any::<u32>(),
    ) {
        let doc = measurement_doc("sig", datatype, Some(addr));
        let project = A2lProject::parse(&doc).unwrap();
        let m = project.modules[0].measurement_by_name("sig").unwrap();
        let dt = DataType::from_keyword(datatype).unwrap();
        prop_assert_eq!(m.datatype, dt);
        prop_assert_eq!(m.datatype.keyword(), dt.keyword());
        prop_assert_eq!(m.datatype.keyword(), datatype);
        prop_assert_eq!(m.datatype.size_bytes(), dt.size_bytes());
    }

    /// Multi-characteristic modules preserve every entry in order; names
    /// drawn from a realistic alphabet never collide with keywords.
    #[test]
    fn multi_characteristic_roundtrip(
        addrs in proptest::collection::vec(any::<u32>(), 0..16),
        seed in any::<u64>(),
    ) {
        let mut doc = String::from("/begin PROJECT p \"d\"\n/begin MODULE m \"d\"\n");
        for (i, addr) in addrs.iter().enumerate() {
            let name = format!("K{seed:04X}_{i:03}");
            doc.push_str(&format!(
                "/begin CHARACTERISTIC {name} \"desc\" VALUE {} RL 0 cm 0 100\n/end CHARACTERISTIC\n",
                render_address(*addr, true)
            ));
        }
        doc.push_str("/end MODULE\n/end PROJECT\n");

        let project = A2lProject::parse(&doc).unwrap();
        let module = &project.modules[0];
        prop_assert_eq!(module.characteristics.len(), addrs.len());
        for (i, addr) in addrs.iter().enumerate() {
            let name = format!("K{seed:04X}_{i:03}");
            let c = module.characteristic_by_name(&name).unwrap();
            prop_assert_eq!(c.address, *addr);
        }
    }

    /// COEFFS / COEFFS_LINEAR round-trip: parsed coefficients reduce to
    /// the expected linear function where the math permits.
    #[test]
    fn coeffs_roundtrip(
        b in -100.0f64..100.0,
        c in -1e6f64..1e6,
        a in -10.0f64..10.0,
    ) {
        let prop_is_linear = a == 0.0;
        let doc = format!(
            "/begin PROJECT p \"d\"\n/begin MODULE m \"d\"\n\
             /begin COMPU_METHOD cm \"d\" RAT_FUNC \"%.3f\" \"x\" COEFFS {a} {b} 1 0 0 1\n\
             /end COMPU_METHOD\n/end MODULE\n/end PROJECT\n"
        );
        let project = A2lProject::parse(&doc).unwrap();
        let cm = project.modules[0].compu_method_by_name("cm").unwrap();
        prop_assert_eq!(
            cm.coeffs,
            a2l_parse::Coeffs::RatFunc([a, b, 1.0, 0.0, 0.0, 1.0])
        );
        match cm.to_linear() {
            Some(lin) => {
                prop_assert!(prop_is_linear);
                prop_assert_eq!(lin.slope, b);
                prop_assert_eq!(lin.intercept, 1.0);
            }
            None => prop_assert!(!prop_is_linear),
        }
    }
}
