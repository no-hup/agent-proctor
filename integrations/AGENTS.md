## Test Proof Verification (`agent-proctor`)

Before finishing a fix or opening a pull request, prove that your tests discriminate between the buggy baseline and your fix:

```bash
proctor verify --base main
```

- **Discriminating**: New test passes on your fix and fails on `main` — your test genuinely proves the fix.
- **DoesNotDiscriminate**: Test passes even without your production changes — tighten assertions so it exercises the change.
- **Broken**: Test fails on your own branch — fix it before wrapping up.

Also run `proctor scan --base main` to confirm existing test assertions and runner configurations remain intact.
