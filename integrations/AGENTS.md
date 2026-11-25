## Test Proof Verification (`agent-proctor`)

Before finishing a fix or opening a pull request, run:

```bash
proctor verify --base main
```

- **Broken**: Fails on your current branch — fix before finishing.
- **Catches change** (`Discriminating` in JSON): Passes on your fix and fails on `main` — shows the test catches the change.
- **Passes on both** (`DoesNotDiscriminate` in JSON): Passes even without your changes — normal for refactor/coverage tests; only review if the test is meant to verify a specific behavior change.

Also run `proctor scan --base main` to confirm existing test assertions and runner configurations remain intact.
