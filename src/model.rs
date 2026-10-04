//! The A2L calibration data model: the parsed subset of the ASAP2 grammar
//! the calibration stack consumes.
//!
//! Field names follow the task-domain vocabulary; the struct fields are
//! the *required* positional grammar elements, with optional ASAP2
//! keywords (e.g. `ECU_ADDRESS` on MEASUREMENT) captured where the model
//! needs them and everything else skipped by the parser.
use alloc::string::String;
use alloc::vec::Vec;

/// A parsed A2L database: the PROJECT name and its MODULEs.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct A2lProject {
    /// PROJECT name.
    pub project: String,
    /// All `/begin MODULE` blocks, in file order.
    pub modules: Vec<A2lModule>,
}

impl A2lProject {
    /// Parse an A2L (ASAP2) description. Keyword-driven and
    /// block-structured: `/begin NAME … /end NAME`; unknown blocks and
    /// unknown optional keywords are skipped, so real-world files with
    /// vendor extensions parse as long as the core blocks are intact.
    ///
    /// # Errors
    ///
    /// [`A2lError`] — typed, with 1-based line numbers where applicable.
    pub fn parse(input: &str) -> Result<Self, crate::A2lError> {
        crate::parser::parse(input)
    }

    /// First module named `name`, in file order.
    #[must_use]
    pub fn module_by_name(&self, name: &str) -> Option<&A2lModule> {
        self.modules.iter().find(|m| m.name == name)
    }
}

/// A `/begin MODULE` block: one ECU's calibration description.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct A2lModule {
    /// MODULE name.
    pub name: String,
    /// CHARACTERISTIC blocks (calibration parameters: values, curves,
    /// maps).
    pub characteristics: Vec<Characteristic>,
    /// MEASUREMENT blocks (ECU-internal quantities to measure).
    pub measurements: Vec<Measurement>,
    /// RECORD_LAYOUT blocks (memory deposit layouts).
    pub record_layouts: Vec<RecordLayout>,
    /// COMPU_METHOD blocks (raw ↔ physical conversion definitions).
    pub compu_methods: Vec<CompuMethod>,
}

impl A2lModule {
    /// The CHARACTERISTIC named `name`.
    #[must_use]
    pub fn characteristic_by_name(&self, name: &str) -> Option<&Characteristic> {
        self.characteristics.iter().find(|c| c.name == name)
    }

    /// The MEASUREMENT named `name`.
    #[must_use]
    pub fn measurement_by_name(&self, name: &str) -> Option<&Measurement> {
        self.measurements.iter().find(|m| m.name == name)
    }

    /// The RECORD_LAYOUT named `name`.
    #[must_use]
    pub fn record_layout_by_name(&self, name: &str) -> Option<&RecordLayout> {
        self.record_layouts.iter().find(|r| r.name == name)
    }

    /// The COMPU_METHOD named `name`.
    #[must_use]
    pub fn compu_method_by_name(&self, name: &str) -> Option<&CompuMethod> {
        self.compu_methods.iter().find(|c| c.name == name)
    }
}

/// A CHARACTERISTIC: one adjustable calibration parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct Characteristic {
    /// CHARACTERISTIC name.
    pub name: String,
    /// Calibration value type (VALUE, CURVE, MAP, VAL_BLK, ASCII).
    pub r#type: CharType,
    /// ECU address of the parameter's deposit.
    pub address: u32,
    /// RECORD_LAYOUT reference (the deposit's memory layout).
    pub deposit: String,
    /// Maximum difference between two adjustment steps.
    pub max_diff: f64,
    /// COMPU_METHOD reference (raw ↔ physical conversion).
    pub conversion: String,
    /// Lower limit of the (physical) value range.
    pub lower_limit: f64,
    /// Upper limit of the (physical) value range.
    pub upper_limit: f64,
}

/// The CHARACTERISTIC type keywords of the ASAP2 grammar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(clippy::exhaustive_enums)]
pub enum CharType {
    /// `VALUE` — a single scalar.
    Value,
    /// `CURVE` — one-dimensional table.
    Curve,
    /// `MAP` — two-dimensional table.
    Map,
    /// `VAL_BLK` — multi-dimensional value block without axes.
    ValBlk,
    /// `ASCII` — a string.
    Ascii,
}

impl CharType {
    /// Parse a CHARACTERISTIC type keyword.
    #[must_use]
    pub fn from_keyword(keyword: &str) -> Option<Self> {
        Some(match keyword {
            "VALUE" => Self::Value,
            "CURVE" => Self::Curve,
            "MAP" => Self::Map,
            "VAL_BLK" => Self::ValBlk,
            "ASCII" => Self::Ascii,
            _ => return None,
        })
    }

    /// The ASAP2 keyword.
    #[must_use]
    pub const fn keyword(self) -> &'static str {
        match self {
            Self::Value => "VALUE",
            Self::Curve => "CURVE",
            Self::Map => "MAP",
            Self::ValBlk => "VAL_BLK",
            Self::Ascii => "ASCII",
        }
    }
}

/// A MEASUREMENT: one ECU-internal quantity available for measurement.
#[derive(Debug, Clone, PartialEq)]
pub struct Measurement {
    /// MEASUREMENT name.
    pub name: String,
    /// ASAP2 datatype of the raw value in ECU memory.
    pub datatype: DataType,
    /// ECU address (`ECU_ADDRESS` keyword; `0` when absent — the value
    /// then lives only in the debugger/trace mapping).
    pub address: u32,
    /// COMPU_METHOD reference (raw ↔ physical conversion).
    pub conversion: String,
}

/// The ASAP2 `DATATYPE` keywords — the raw memory representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(clippy::exhaustive_enums)]
pub enum DataType {
    /// `UBYTE` — unsigned 8-bit.
    Ubyte,
    /// `SBYTE` — signed 8-bit.
    Sbyte,
    /// `UWORD` — unsigned 16-bit.
    Uword,
    /// `SWORD` — signed 16-bit.
    Sword,
    /// `ULONG` — unsigned 32-bit.
    Ulong,
    /// `SLONG` — signed 32-bit.
    Slong,
    /// `A_UINT64` — unsigned 64-bit.
    Uint64,
    /// `A_INT64` — signed 64-bit.
    Int64,
    /// `FLOAT32_IEEE` — IEEE-754 single precision.
    Float32,
    /// `FLOAT64_IEEE` — IEEE-754 double precision.
    Float64,
}

impl DataType {
    /// Parse an ASAP2 `DATATYPE` keyword.
    #[must_use]
    pub fn from_keyword(keyword: &str) -> Option<Self> {
        Some(match keyword {
            "UBYTE" => Self::Ubyte,
            "SBYTE" => Self::Sbyte,
            "UWORD" => Self::Uword,
            "SWORD" => Self::Sword,
            "ULONG" => Self::Ulong,
            "SLONG" => Self::Slong,
            "A_UINT64" => Self::Uint64,
            "A_INT64" => Self::Int64,
            "FLOAT32_IEEE" => Self::Float32,
            "FLOAT64_IEEE" => Self::Float64,
            _ => return None,
        })
    }

    /// The ASAP2 keyword.
    #[must_use]
    pub const fn keyword(self) -> &'static str {
        match self {
            Self::Ubyte => "UBYTE",
            Self::Sbyte => "SBYTE",
            Self::Uword => "UWORD",
            Self::Sword => "SWORD",
            Self::Ulong => "ULONG",
            Self::Slong => "SLONG",
            Self::Uint64 => "A_UINT64",
            Self::Int64 => "A_INT64",
            Self::Float32 => "FLOAT32_IEEE",
            Self::Float64 => "FLOAT64_IEEE",
        }
    }

    /// Size of the raw value in ECU memory, in bytes.
    #[must_use]
    pub const fn size_bytes(self) -> usize {
        match self {
            Self::Ubyte | Self::Sbyte => 1,
            Self::Uword | Self::Sword => 2,
            Self::Ulong | Self::Slong | Self::Float32 => 4,
            Self::Uint64 | Self::Int64 | Self::Float64 => 8,
        }
    }
}

/// A RECORD_LAYOUT: how a parameter's values deposit into ECU memory.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordLayout {
    /// RECORD_LAYOUT name.
    pub name: String,
    /// The `FNC_VALUES` description when present.
    pub fnc_values: Option<FncValues>,
}

/// The RECORD_LAYOUT `FNC_VALUES` keyword: where and how the value array
/// deposits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FncValues {
    /// `POSITION` — the array's start offset in the deposit record.
    pub position: u8,
    /// `DATATYPE` — the element datatype.
    pub datatype: DataType,
}

/// A COMPU_METHOD: the raw ↔ physical conversion for one quantity.
#[derive(Debug, Clone, PartialEq)]
pub struct CompuMethod {
    /// COMPU_METHOD name.
    pub name: String,
    /// Conversion type (LINEAR, RAT_FUNC, TABLE, IDENTITY).
    pub conversion_type: ConversionType,
    /// Coefficients carried by the method (`COEFFS`/`COEFFS_LINEAR`);
    /// [`Coeffs::None`] for TABLE/IDENTITY and for methods that reference
    /// a table via `COMPU_TAB_REF` instead.
    pub coeffs: Coeffs,
    /// The `COMPU_TAB_REF` table name when the conversion is tabular.
    pub tab_ref: Option<String>,
    /// Physical unit string (may be empty).
    pub unit: String,
}

impl CompuMethod {
    /// Reduce the method to a linear raw → physical function when the
    /// conversion is linear-or-reducibly-rational:
    ///
    /// - `LINEAR` (`COEFFS_LINEAR a b`): `f(x) = a·x + b` directly.
    /// - `RAT_FUNC` with `d = e = 0` and `f ≠ 0`:
    ///   `(a·x² + b·x + c) / f` reduces to linear with slope
    ///   `a / f`, intercept `c / f` — the form nearly every real ECU uses.
    /// - `IDENTITY`: `f(x) = x`.
    /// - `TABLE` (and non-linear `RAT_FUNC`): not reducible → `None`.
    // The COEFFS destructure below uses the spec's own a b c d e f
    // notation; single-char names are the domain vocabulary here.
    #[allow(clippy::many_single_char_names)]
    #[must_use]
    pub fn to_linear(&self) -> Option<LinearFn> {
        match self.conversion_type {
            ConversionType::Identity => Some(LinearFn {
                slope: 1.0,
                intercept: 0.0,
            }),
            ConversionType::Linear => match self.coeffs {
                Coeffs::Linear([a, b]) => Some(LinearFn {
                    slope: a,
                    intercept: b,
                }),
                _ => None,
            },
            ConversionType::RatFunc => match self.coeffs {
                // Reducibly rational: constant denominator (d = e = 0,
                // f ≠ 0) and a *linear* numerator (a = 0), i.e.
                // (b·x + c) / f. A non-zero x² term is genuinely
                // non-linear and stays `None`.
                Coeffs::RatFunc([a, b, c, d, e, f])
                    if a == 0.0 && d == 0.0 && e == 0.0 && f != 0.0 =>
                {
                    Some(LinearFn {
                        slope: b / f,
                        intercept: c / f,
                    })
                }
                _ => None,
            },
            ConversionType::Table => None,
        }
    }
}

/// A linear raw → physical function `f(x) = slope·x + intercept`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinearFn {
    /// Multiplicative term `a`.
    pub slope: f64,
    /// Additive term `b`.
    pub intercept: f64,
}

impl LinearFn {
    /// Apply the conversion to a raw value.
    #[must_use]
    pub fn apply(&self, raw: f64) -> f64 {
        self.slope * raw + self.intercept
    }
}

/// The COMPU_METHOD coefficient forms.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[allow(clippy::exhaustive_enums)]
pub enum Coeffs {
    /// `COEFFS a b c d e f` — rational conversion
    /// `f(x) = (a·x² + b·x + c) / (d·x² + e·x + f)`.
    RatFunc([f64; 6]),
    /// `COEFFS_LINEAR a b` — linear conversion `f(x) = a·x + b`.
    Linear([f64; 2]),
    /// No coefficients present (TABLE via `COMPU_TAB_REF`, IDENTITY, or
    /// an abbreviated method).
    #[default]
    None,
}

/// The COMPU_METHOD conversion-type keywords.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(clippy::exhaustive_enums)]
pub enum ConversionType {
    /// `LINEAR` — `COEFFS_LINEAR a b`.
    Linear,
    /// `RAT_FUNC` — `COEFFS a b c d e f`.
    RatFunc,
    /// `TABLE` — conversion via a `COMPU_TAB_REF` table.
    Table,
    /// `IDENTITY` (also accepted spelled `IDENTICAL`) — raw = physical.
    Identity,
}

impl ConversionType {
    /// Parse a conversion-type keyword (`IDENTICAL` is accepted as an
    /// alias of `IDENTITY`).
    #[must_use]
    pub fn from_keyword(keyword: &str) -> Option<Self> {
        Some(match keyword {
            "LINEAR" => Self::Linear,
            "RAT_FUNC" => Self::RatFunc,
            "TABLE" => Self::Table,
            "IDENTITY" | "IDENTICAL" => Self::Identity,
            _ => return None,
        })
    }

    /// The canonical ASAP2 keyword.
    #[must_use]
    pub const fn keyword(self) -> &'static str {
        match self {
            Self::Linear => "LINEAR",
            Self::RatFunc => "RAT_FUNC",
            Self::Table => "TABLE",
            Self::Identity => "IDENTITY",
        }
    }
}
