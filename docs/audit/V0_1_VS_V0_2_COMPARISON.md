# Comparative Audit Report: Proctor v0.1.0 vs. v0.2.0-dev

*Historical replay of `proctor audit` over 5 open-source repositories (449 commits). v0.1.0 = tag `v0.1.0`. v0.2.0-dev = commit `224d2c4`.*

> **Status: preliminary.** These numbers were measured on `224d2c4`. The follow-up commit (tighter linkage, body-symbol deletion justification, hardened rename detection, removed-strictness detection) changes PROCTOR-001 and PROCTOR-004 behavior and has **not** been re-audited yet. No finding below has been hand-labeled. "Finding" counts are not precision.

## 1. Measured results

| Repository | Commits | v0.1.0 clean % | v0.2.0-dev clean % | v0.1.0 blocked commits | v0.2.0-dev blocked commits |
| :--- | :---: | :---: | :---: | :---: | :---: |
| `aider-AI/aider` | 100 | 94.0% | 94.0% | 6 | 6 |
| `princeton-nlp/SWE-agent` | 100 | 98.0% | 98.0% | 0 | 0 |
| `browser-use/browser-use` | 100 | 56.0% | 55.0% | 28 | 31 |
| `OpenHands/OpenHands` | 100 | 64.0% | 64.0% | 11 | 11 |
| `BerriAI/litellm` | 49 | 57.1% | 57.1% | 14 | 15 |

Findings by rule (individual findings, not commits):

| Repository | Rule | v0.1.0 | v0.2.0-dev |
| :--- | :--- | :---: | :---: |
| aider | PROCTOR-001 Assertion lock | 11 | 11 |
| SWE-agent | PROCTOR-001 Assertion lock | 2 | 2 |
| browser-use | PROCTOR-001 Assertion lock | 122 | 125 |
| browser-use | PROCTOR-002 Internal mock | 18 | 18 |
| OpenHands | PROCTOR-001 Assertion lock | 182 | 182 |
| OpenHands | PROCTOR-002 Internal mock | 13 | 17 |
| litellm | PROCTOR-001 Assertion lock | 695 | 896 |
| litellm | PROCTOR-002 Internal mock | 73 | 76 |
| litellm | PROCTOR-006 Parametrize row drop | 3 | 3 |

## 2. What the data does and does not show

**The noise target was missed.** The v0.2 plan targeted a ≥70% reduction in false flags on OpenHands and litellm, with clean baselines rising toward 85%. Clean rates did not move (64.0% and 57.1%). The cause is structural: most of the noise is report-tier PROCTOR-001 assertion *modifications* (182 on OpenHands, 695+ on litellm). Static linkage decides whether such a finding is Review or Block. It does not remove it. Reducing this noise needs a separate change, such as classifying modifications that follow a linked production change, and hand-labeled evidence that the classification is right.

**No regression on the quiet repos.** aider and SWE-agent produced identical counts.

**browser-use: +3 blocked commits.** This matches the intended mechanism. Under v0.1.0, any production file in the diff demoted a weakened assertion to Review. Under v0.2.0-dev, demotion needs the test to reference a changed definition. The three commits have **not** been hand-labeled, so we do not call them true positives. Even if all three are real, it is 3 of 100 commits in 1 of 5 repos: the demotion loophole was rarely decisive in human commit history. Its importance is adversarial (agents optimizing to pass), which organic history does not measure.

**litellm: PROCTOR-001 +201 findings, blocked commits +1.** Linkage changes a finding's tier, not whether it exists, so it cannot explain a +201 count change. The most likely source is the stricter whole-test deletion rule (`.all()` instead of `.any()`), which `224d2c4` applied to assertion symbols only; the follow-up commit switches it to body symbols and should be re-measured. This delta is unexplained until the finding-ID diff below is done.

**PROCTOR-002 mock deltas (OpenHands +4, litellm +3) are unexplained.** The shadow-mock rule did not change between the two versions. An earlier draft of this document attributed the delta to new mock rules; no such rules exist, and that claim has been removed. One candidate cause: `is_in_repo_target` checks file existence relative to the working directory, so results can depend on checkout state during the audit. Until verified, these deltas should not be cited as catches.

## 3. Required before citing any of these numbers

1. Re-run both binaries on the same commit windows with the current `main`, emitting JSON per commit.
2. Diff by **finding ID** (stable Blake3 content hash): report added, removed, and tier-changed sets per rule.
3. Hand-label at least 20 findings from each added set (or all, if fewer) as true positive / false positive.
4. Rewrite sections 1–2 from those labels.

## 4. What changed in the engine (for reference)

1. **Assertion-definition linkage.** A weakened assertion is demoted to Review only when the test references a production definition changed in the PR. Definitions are module-level and class-member only; test-local variables are ignored.
2. **Whole-test deletion.** Justified only if every production symbol the deleted test touches (calls, mock-style checks, assertions) was removed in the PR.
3. **Intra-file rename detection.** Ordered token-bigram similarity ≥ 0.75, bodies of at least 8 tokens, ties rejected. A renamed symbol is not "deleted", so it cannot justify a weakening.
4. **Config loosening (PROCTOR-004) and blanket suppressions (PROCTOR-008).** Strictness flags turned off or removed and coverage floors lowered or removed are blocked. Per-module overrides, new config files, and lint-ignore additions are Review.
5. **Test-Change Receipt.** Markdown summary for `$GITHUB_STEP_SUMMARY`, including a "Not analyzed" row for test files in unsupported languages. It states that the check is static.
