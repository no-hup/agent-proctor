# Track C Live-Runner Verification Matrix (`proctor verify`)

Verified with compiled release binary (`agent-proctor` commit `58440fc`) against **live** `uv` + `pytest`, `uv pip install -e .` (`src/` editable install), and `npm` + `jest`.

## 1. Live Test Outcome Matrix

| Repo / Environment | Test Case (`test_id`) | Expected | Observed Status | Back-to-Back JSON Identical? |
| :--- | :--- | :--- | :--- | :---: |
| **Flat Python (`uv` + `pytest`)** | `tests/test_all.py::test_discriminating` | `Discriminating` | `Discriminating` (`assert -1 == 5`) | ✅ |
| **Flat Python (`uv` + `pytest`)** | `tests/test_all.py::test_does_not_discriminate` | `DoesNotDiscriminate` | `DoesNotDiscriminate` | ✅ |
| **Flat Python (`uv` + `pytest`)** | `tests/test_all.py::test_broken` | `Broken` | `Broken` (`assert 5 == 999`) | ✅ |
| **Flat Python (PR Added Module)** | `tests/test_all.py::test_added_module_missing_on_base` | `MissingOnBase` | `MissingOnBase` (`ModuleNotFoundError: No module named 'app.newmod'`) | ✅ |
| **Flat Python (PR Deleted Module)** | `tests/test_all.py::test_deleted_module_was_present_on_base` | `Discriminating` | `Discriminating` (`assert 42 == 999`) | ✅ |
| **`src/` Editable (`uv pip install -e .`)** | `tests/test_calc.py::test_discriminating` | `Discriminating` | `Discriminating` (`assert 4 == 8`) | ✅ |
| **`src/` Editable (`uv pip install -e .`)** | `tests/test_calc.py::test_does_not_discriminate` | `DoesNotDiscriminate` | `DoesNotDiscriminate` | ✅ |
| **`src/` Editable (`uv pip install -e .`)** | `tests/test_calc.py::test_broken` | `Broken` | `Broken` (`assert 8 == 999`) | ✅ |
| **Real Jest (`npm install jest`)** | `tests/calc.test.js::discriminating test` | `Discriminating` | `Discriminating` (`expect(received).toBe(expected)`) | ✅ |
| **Real Jest (`npm install jest`)** | `tests/calc.test.js::non discriminating test` | `DoesNotDiscriminate` | `DoesNotDiscriminate` | ✅ |
| **Real Jest (`npm install jest`)** | `tests/calc.test.js::broken test` | `Broken` | `Broken` (`expect(received).toBe(expected)`) | ✅ |
| **Live Vitest (`npm install vitest`)** | `tests/calc.test.js::discriminating vitest` | `Discriminating` | `Discriminating` (`AssertionError: expected -1 to be 5`) | ✅ |
| **Live Vitest (`npm install vitest`)** | `tests/calc.test.js::non discriminating vitest` | `DoesNotDiscriminate` | `DoesNotDiscriminate` | ✅ |
| **Live Vitest (`npm install vitest`)** | `tests/calc.test.js::broken vitest` | `Broken` | `Broken` (`AssertionError: expected 5 to be 99`) | ✅ |
| **Crash Isolation (`pytest`)** | `TypeError` & `KeyError` on base | `Inconclusive` | `Inconclusive` (`Non-assertion failure on base`) | ✅ |
| **Crash Isolation (`jest`)** | `TypeError` on base | `Inconclusive` | `Inconclusive` (`Non-assertion failure on base`) | ✅ |
| **Crash Isolation (`vitest`)** | `TypeError` on base | `Inconclusive` | `Inconclusive` (`Non-assertion failure on base`) | ✅ |

---

## 2. Safety & Interrupt Matrix

| Safety Check | Procedure Tested | Verified Result |
| :--- | :--- | :--- |
| **Mid-swap `SIGKILL` recovery** | `kill -9` while `.git/proctor-swap-backup.json` was active during base pass | Subsequent `verify` refused; `proctor verify --restore` restored 1/1 file and left `git status` clean |
| **Dirty working tree with `--head`** | Edited tracked `app/math.py` without committing and ran `verify --base ... --head HEAD` | Refused cleanly with exit error before mutating disk |
| **Zero-network Jest / Vitest** | Always calls `node_modules/.bin/{jest,vitest}` with `--no-cache` | No `npx` invocation; cache disabled |
| **Pytest cache / bytecode isolation** | Runs `python -B -m pytest -p no:randomly -p no:cacheprovider` + purges swapped `.pyc` | Base swaps always read updated source files |
| **Clean host working tree** | Checked `git status --short` after every repo test | Clean (zero leftover swap artifacts) |

---

## 3. Real-Runner Bugs Caught & Fixed During Track C

1. **Flat Python `PYTHONPATH` (`app.math` collection failure):** `.venv/bin/pytest` omitted `repo_root` from `sys.path`. Fixed by preferring `.venv/bin/python -B -m pytest` and prepending `repo_root` to `PYTHONPATH`.
2. **Pytest JUnit XML assertion detection:** Real `pytest --junitxml` omits `type="AssertionError"` on `<failure>` tags and encodes newlines as `&#10;` with `message="assert ..."`. Updated `parse_junit_xml` and `decode_xml_entities` while still rejecting `TypeError`/`KeyError`/`NameError`.
3. **JSON output determinism & pure stdout:** Excluded wall-clock `duration_ms` from JSON serialization (`#[serde(skip_serializing)]`) and routed progress banners to `stderr` so `proctor verify --format json` is 100% pure, byte-identical JSON across consecutive runs.
