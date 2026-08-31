# Capability ledger

`capability-ledger.json` is the source of truth for the office-suite roadmap. It deliberately separates implementation from proof:

- `absent`: no implementation exists.
- `partial`: an entry point or subset exists, but important behavior is missing.
- `implemented`: focused automated tests cover the behavior.
- `verified`: a repeatable end-to-end or fixture-based check has passed.
- `interoperable`: verification also proves the relevant Office-format round trip.

Every feature change must update its ledger entry and add evidence in the same slice. Evidence should identify a source module, test, fixture, benchmark, or manual acceptance record. A visible toolbar button or a passing type-check is not sufficient evidence.

The validator is intentionally dependency-free:

```powershell
node scripts/validate-capability-ledger.mjs
```

Future quality-gate work should run this validator before the existing build, test, accessibility, and performance gates.
