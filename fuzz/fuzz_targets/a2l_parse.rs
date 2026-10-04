//! Fuzz target — `A2lProject::parse` totality: arbitrary input produces
//! either a parsed project or a typed error, never a panic. When parsing
//! succeeds, lookups over the extracted model must agree with the model.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    if let Ok(project) = a2l_parse::A2lProject::parse(data) {
        // The model is internally consistent: every module answers lookups
        // over its own vecs, and every characteristic's datatype-relevant
        // fields are well-formed.
        for module in &project.modules {
            for c in &module.characteristics {
                assert_eq!(
                    module.characteristic_by_name(&c.name).map(|found| found.address),
                    Some(c.address)
                );
                let _ = c.r#type.keyword();
            }
            for m in &module.measurements {
                assert_eq!(
                    module.measurement_by_name(&m.name).map(|found| found.address),
                    Some(m.address)
                );
                let _ = m.datatype.keyword();
                let _ = m.datatype.size_bytes();
            }
            for cm in &module.compu_methods {
                assert_eq!(
                    module.compu_method_by_name(&cm.name).map(|found| found.conversion_type),
                    Some(cm.conversion_type)
                );
                // to_linear must exist for identity and never be Some for
                // tables.
                match cm.conversion_type {
                    a2l_parse::ConversionType::Table => assert!(cm.to_linear().is_none()),
                    a2l_parse::ConversionType::Identity => assert!(cm.to_linear().is_some()),
                    _ => {}
                }
            }
        }
    }
});
