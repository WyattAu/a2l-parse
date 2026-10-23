# a2l-parse
[![Estate integration](https://github.com/WyattAu/estate-integration/actions/workflows/ci.yml/badge.svg)](https://github.com/WyattAu/estate-integration/actions/workflows/ci.yml)

ASAP2/A2L ECU description parser: PROJECT/MODULE/CHARACTERISTIC/
MEASUREMENT/RECORD_LAYOUT/COMPU_METHOD, calibration data model.

**Layer:** L1: substrate · **Estate deps:** zero · **Runtime deps:**
none · `no_std` + `alloc` (CI exercises `thumbv7em-none-eabihf`).

## What's here

| Module | Contents |
|---|---|
| `model` | [`A2lProject::parse`](https://docs.rs/a2l-parse/latest/a2l_parse/struct.A2lProject.html): the parsed database (`project`, `modules`); `A2lModule` with `characteristic_by_name` / `measurement_by_name` (plus record-layout and compu-method lookups); `Characteristic` (`r#type`: Value/Curve/Map/ValBlk/Ascii, address, deposit, max_diff, conversion, limits); `Measurement` (datatype, `ECU_ADDRESS`, conversion); `RecordLayout` (`FNC_VALUES` position + datatype); `CompuMethod` (LINEAR/RAT_FUNC/TABLE/IDENTITY, `COEFFS`/`COEFFS_LINEAR`/`COMPU_TAB_REF`, unit) with `to_linear()`: the raw→physical reduction every calibration consumer wants |
| `lexer` | Hand-rolled scanner: identifiers, `/begin`-style keywords, decimal/hex/float literals, quoted strings with C-style escapes (multi-byte UTF-8 safe), `/* */` and `//` comments: `get()`-only byte access, total by construction |
| `parser` | Keyword-driven, block-structured recursive descent: consumes the required positionals of the six core blocks, captures the optional keywords the model needs, and **skips** everything else: unknown optional keywords, unknown `/begin … /end` sub-blocks (AXIS_DESCR, IF_DATA braces, vendor extensions) |
| `error` | `A2lError`: typed, exhaustive, with 1-based line numbers |

## Guarantees

- **Never panics**: every byte access is `get()`-based (deny-level
  `unwrap`/`expect`/`panic`/`indexing_slicing` lints on the lib target);
  multi-byte UTF-8 inside strings is boundary-safe (property of the
  scanner, regression-tested and fuzz-found).
- **Skip-tolerant**: real-world files with vendor blocks (`IF_DATA`,
  `AXIS_DESCR`, `FUNCTION`, `GROUP`, `COMPU_VTAB`, OEM extensions) parse
  as long as the core blocks are intact. Positional runs are strict and
  produce typed errors.
- **Bounded**: `/begin` nesting deeper than [`MAX_DEPTH`](https://docs.rs/a2l-parse/latest/a2l_parse/#constants)
  is a typed error, never a stack overflow.
- **Compu-method math**: `to_linear()` reduces `RAT_FUNC` only when the
  conversion is genuinely linear (`d = e = a = 0`, `f ≠ 0`: slope from
  the x-coefficient), applies `COEFFS_LINEAR` directly, and treats
  `IDENTITY` as `f(x) = x`; TABLE and irreducibly-rational methods return
  `None` rather than a wrong approximation.

## Example

```rust
use a2l_parse::{A2lProject, CharType, DataType};

let project = A2lProject::parse(r#"
    /begin PROJECT demo "Demonstration project"
      /begin MODULE engine "V8 ECU"
        /begin CHARACTERISTIC eng_load "Engine load" VALUE 0x720000
          RL_VALUE 0 rack_lin 0.0 100.0
        /end CHARACTERISTIC
        /begin MEASUREMENT engine_speed "Engine speed" UWORD
          engine_speed_c 0.01 0.0 0.0 8000.0
          ECU_ADDRESS 0x320001
        /end MEASUREMENT
        /begin COMPU_METHOD rack_lin "Rack position" RAT_FUNC
          "%.4f" "%" COEFFS 0 0.00390625 0 0 0 1
        /end COMPU_METHOD
      /end MODULE
    /end PROJECT
"#).unwrap();

let module = project.module_by_name("engine").unwrap();
let kfile = module.characteristic_by_name("eng_load").unwrap();
assert_eq!(kfile.r#type, CharType::Value);
assert_eq!(kfile.address, 0x720000);

let meas = module.measurement_by_name("engine_speed").unwrap();
assert_eq!(meas.datatype, DataType::Uword);
assert_eq!(meas.address, 0x320001);

// Raw → physical: (0·x² + 0.00390625·x + 0) / 1.
let lin = module.compu_method_by_name("rack_lin").unwrap().to_linear().unwrap();
assert_eq!(lin.apply(256.0), 1.0);
```

## Tests

`cargo test` runs unit + integration suites, doctests, and a 5-property
500-case proptest battery (u32 address round-trips through
CHARACTERISTIC and MEASUREMENT in hex/decimal, datatype round-trips,
multi-characteristic ordering, coefficient reduction). `cargo fuzz run
a2l_parse` covers the parser (it caught and fixed a UTF-8 boundary bug
before release); miri runs the lib suite; `thumbv7em-none-eabihf` is
checked in CI.

## License

MIT OR Apache-2.0: see LICENSE-MIT and LICENSE-APACHE.
