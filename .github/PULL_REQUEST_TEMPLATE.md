## Description
<!-- Briefly describe the changes made in this PR. -->

## Rule & Precision Impact
<!-- If modifying rules or Tree-sitter parsers, describe the impact on recall and false positives. -->
- [ ] New clean fixtures added under `corpus/clean/` to verify zero false positives
- [ ] New attack fixtures added under `corpus/<language>/` if adding/modifying anticheat rules
- [ ] All 43+ existing unit and corpus integration tests pass (`cargo test --all`)
- [ ] Clippy check passes with zero warnings (`cargo clippy --all -- -D warnings`)

## Verification Checklist
- [ ] `cargo test --all` passed
- [ ] Self-scanned with `cargo run --bin proctor -- scan --base HEAD~1`
