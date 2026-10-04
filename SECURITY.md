# Security Policy — a2l-parse

## Supported versions

| Version | Supported |
|---------|-----------|
| 0.1.x   | ✅        |

## Reporting a vulnerability

Report privately via [GitHub security advisories] for this repository, or
email **wyatt_au@protonmail.com**. Do **not** open a public issue for
security reports.

You will receive an acknowledgement within **72 hours**. Coordinated
disclosure: we ask for up to 90 days before public disclosure while a
patch ships.

[GitHub security advisories]: https://github.com/WyattAu/a2l-parse/security/advisories/new

## Threat model

**I/O surface: untrusted input.** `a2l-parse` parses ECU description
files that originate from suppliers, version control, ticket systems, and
other semi-trusted sources. The trust boundary is the *contents* of every
`&str` handed to `A2lProject::parse`. Callers control where the text
comes from; a2l-parse guarantees safety of the *parse* regardless.

| ID | Threat | Mitigation |
|----|--------|------------|
| T1 | Malformed/truncated files drive the parser out of bounds or into a panic (including UTF-8 char-boundary slicing on multi-byte input) | Totality by construction: `get()`-only byte access; `unwrap`/`expect`/`panic`/`indexing_slicing` denied at the lib target; exhaustive `A2lError`; multi-byte string handling is boundary-safe and fuzz-verified; cargo-fuzz target `a2l_parse`, 30 s CI fuzz smoke, miri over the lib suite |
| T2 | Deeply nested `/begin` blocks exhaust the stack (recursive descent bomb) | `MAX_DEPTH` (32) nesting guard raises a typed `DepthLimitExceeded` instead of overflowing |
| T3 | Unknown block/keyword constructs desynchronize block counting, mis-attributing calibration addresses | Slash-keyword-aware balanced skip: `/begin`/`/end` are counted across all block keywords; strings are atomic tokens so `/begin` inside a description cannot forge structure |
| T4 | Wrong compu-method reduction silently corrupts physical values | `to_linear()` reduces only provably-linear forms (x² term zero, constant non-zero denominator); TABLE and irreducibly-rational methods return `None` instead of an approximation |
| T5 | Dependency-supply compromise | Zero runtime dependencies; proptest is dev-only; `cargo-deny` advisories/bans gate in CI; `cargo-vet` supply-chain directory committed |
