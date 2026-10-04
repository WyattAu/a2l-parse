# Changelog

All notable changes to this project will be documented in this file.
Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versioning:
[SemVer](https://semver.org/).

## [0.1.0] — 2026-10-04

Initial release — L1 substrate, zero estate deps, zero runtime deps,
`no_std` + `alloc`.

### Added

- **Data model**: `A2lProject::parse` — PROJECT/MODULE hierarchy with
  `module_by_name`; `Characteristic` (Value/Curve/Map/ValBlk/Ascii,
  address, deposit, max_diff, conversion, limits) with
  `characteristic_by_name`; `Measurement` (all ten ASAP2 datatypes,
  `ECU_ADDRESS`, conversion) with `measurement_by_name`; `RecordLayout`
  (`FNC_VALUES` position/datatype); `CompuMethod`
  (LINEAR/RAT_FUNC/TABLE/IDENTITY, `COEFFS`, `COEFFS_LINEAR`,
  `COMPU_TAB_REF`, unit) with `record_layout_by_name` /
  `compu_method_by_name` and `to_linear()` raw→physical reduction.
- **Parser**: keyword-driven, block-structured (`/begin … /end`) with
  skip-tolerance for unknown optional keywords, unknown sub-blocks, and
  `IF_DATA`-style brace bodies; `MAX_DEPTH` nesting guard; typed errors
  with 1-based line numbers ([`A2lError`] — 12 exhaustive variants incl.
  `UnterminatedString`, `UnterminatedBlock`, `BlockMismatch`,
  `InvalidAddress`, `DepthLimitExceeded`).
- **Scanner**: hand-rolled lexer over `core` — decimal/hex/float literals
  with exponents, quoted strings with C-style escapes (multi-byte UTF-8
  safe), `/* */` and `//` comments, `get()`-only byte access.
- **Gates**: 5-property 500-case proptest battery, cargo-fuzz target
  `a2l_parse` with committed corpus (found + fixed a UTF-8 boundary bug
  pre-release), miri, embedded (`thumbv7em-none-eabihf`), tier-a shared
  CI pinned to engineering-standards `main`, 93.8 % line coverage.

[0.1.0]: https://github.com/WyattAu/a2l-parse/releases/tag/v0.1.0
