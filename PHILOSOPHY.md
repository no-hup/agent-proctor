# PHILOSOPHY.md — Engineering & Architecture Invariants

This document defines the architectural discipline of `agent-proctor`. Every contributor agent and human developer working on this codebase must adhere strictly to these principles.

---

## 1. Deep Modules, Tiny Seams
- **A lot of behavior behind a small interface.** The caller passes minimal context and gets an authoritative answer. Complex parsing, AST traversals, and path normalizations must remain completely hidden inside the module.
- **The Two-Adapter Rule:** Never create an abstraction, trait, or interface unless at least **two** real implementations vary across it (e.g. baseline vs candidate, production vs test). A single-adapter seam is just indirection bloat.

## 2. Decoupled Black Boxes for High-Variance Logic
- Core diffing, AST parsing, and Blake3 hashing are stable foundations—never churn them.
- High-variance heuristics (Code-to-Test Linkage, Dynamic Escalation, Finding Waivers) must be decoupled behind clean, swappable Rust traits (`LinkageStrategy`, `EscalatorStrategy`, `WaiverStore`).
- Never hardcode speculative heuristics into core AST diffing. Keep implementations modular and isolated.

## 3. Empirical Benchmark-Driven Promotion
- **Numbers decide, not subjective debate.**
- Before any new strategy replaces a default, it must be evaluated side-by-side against the baseline across our 439-PR real-world test suite (`scale_agent_pr_sweep.py`).
- A candidate strategy is promoted to default only if it:
  1. Reduces false-positive noise on legitimate migrations/refactors.
  2. Preserves 100% recall on genuine adversarial test tampering.
  3. Introduces zero latency regression (<50ms static scan).

## 4. Evidence Over Erasure (Auditability First)
- **An automated gate must never decide human intent.**
- If an assertion or test contract changes, surface the exact before/after evidence in the receipt.
- Dynamic verification probes must **annotate evidence**, never erase findings. A passing probe on head and failing on base proves sensitivity, not business authorization.

## 5. The Deletion Test
- If an experimental strategy, rule, or heuristic fails to strictly beat the baseline on the benchmark suite, delete it immediately.
- If deleting a module makes complexity vanish, it was a pass-through—delete it. If complexity reappears across $N$ callers, it earned its keep.

## 6. The Hermetic & Speed Invariants
- **Zero LLMs in the Gate:** 100% deterministic, reproducible AST and byte diffing only.
- **Zero Network Egress:** The engine runs fully offline in CI and local git hooks.
- **Sub-50ms Static Latency:** The scanner must remain instantaneous for developers.
