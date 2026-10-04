//! ASAP2/A2L ECU description parser — PROJECT/MODULE/CHARACTERISTIC/
//! MEASUREMENT/RECORD_LAYOUT/COMPU_METHOD, calibration data model.
//!
//! `a2l-parse` is the estate's **L1 substrate** for the description half
//! of the calibration stack: it turns A2L (ASAM MCD-2 MC / ASAP2) ECU
//! description files into a typed data model that pairs with `xcp-core`'s
//! memory access (UPLOAD/DOWNLOAD to the addresses found here).
//!
//! # Skip-tolerant, total-ish parsing
//!
//! The grammar is keyword-driven and block-structured (`/begin NAME …
//! /end NAME`). The parser consumes the required positional parameters of
//! the six core blocks, captures the optional keywords the model needs
//! (`ECU_ADDRESS`, `FNC_VALUES`, `COEFFS`, `COEFFS_LINEAR`,
//! `COMPU_TAB_REF`), and **skips** everything else — unknown optional
//! keywords, unknown `/begin` sub-blocks ([`AXIS_DESCR`-style vendor
//! extensions]), `IF_DATA` bodies with braces. Real-world files with
//! vendor extensions parse as long as the core blocks are intact. Malformed
//! input resolves to a typed [`A2lError`] with a 1-based line number —
//! never a panic (deny-level lints on the lib target, a cargo-fuzz target,
//! miri).
//!
//! # Example
//!
//! ```
//! use a2l_parse::{A2lProject, CharType, CompuMethod, ConversionType, DataType};
//!
//! let a2l = r#"
//!     /begin PROJECT demo "Demonstration project"
//!       /begin MODULE engine "V8 ECU"
//!         /begin CHARACTERISTIC eng_load "Engine load" VALUE 0x720000
//!           RL_VALUE 0 rack_lin 0.0 100.0
//!         /end CHARACTERISTIC
//!         /begin MEASUREMENT engine_speed "Engine speed" UWORD
//!           engine_speed_c 0.01 0.0 0.0 8000.0
//!           ECU_ADDRESS 0x320001
//!         /end MEASUREMENT
//!         /begin COMPU_METHOD rack_lin "Rack position" RAT_FUNC
//!           "%.4f" "%" COEFFS 0 0.00390625 0 0 0 1
//!         /end COMPU_METHOD
//!       /end MODULE
//!     /end PROJECT
//! "#;
//! let project = A2lProject::parse(a2l)?;
//!
//! assert_eq!(project.project, "demo");
//! assert_eq!(project.modules.len(), 1);
//!
//! let module = project.module_by_name("engine").unwrap();
//! let kfile = module.characteristic_by_name("eng_load").unwrap();
//! assert_eq!(kfile.r#type, CharType::Value);
//! assert_eq!(kfile.address, 0x720000);
//! assert_eq!(kfile.deposit, "RL_VALUE");
//! assert_eq!(kfile.lower_limit, 0.0);
//! assert_eq!(kfile.upper_limit, 100.0);
//!
//! let meas = module.measurement_by_name("engine_speed").unwrap();
//! assert_eq!(meas.datatype, DataType::Uword);
//! assert_eq!(meas.address, 0x320001);
//!
//! // Raw → physical conversion of the RAT_FUNC method:
//! let cm = module.compu_method_by_name("rack_lin").unwrap();
//! assert_eq!(cm.conversion_type, ConversionType::RatFunc);
//! let lin = cm.to_linear().unwrap();
//! assert_eq!(lin.apply(256.0), 1.0);
//! # Ok::<(), a2l_parse::A2lError>(())
//! ```
//!
//! # `no_std`
//!
//! The crate is `no_std` + `alloc` with **zero runtime dependencies** —
//! the scanner, parser, and error type are hand-rolled over `core` +
//! `alloc`.

// ASAP2/A2L grammar keywords (CHARACTERISTIC, COMPU_METHOD,
// ECU_ADDRESS …) are proper nouns of the format; backticking
// every occurrence would bury the prose. Allowed with this
// justification per the fleet pedantic policy.
#![allow(clippy::doc_markdown)]
#![no_std]
extern crate alloc;

mod error;
mod lexer;
mod model;
mod parser;

pub use error::A2lError;
pub use model::{
    A2lModule, A2lProject, CharType, Characteristic, Coeffs, CompuMethod, ConversionType, DataType,
    FncValues, LinearFn, Measurement, RecordLayout,
};
