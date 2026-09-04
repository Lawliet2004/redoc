# Roadmap vocabulary

The repo historically carried three overlapping phase vocabularies. This table is the
authoritative mapping. The **capability ledger's `targetPhase` values are operative** —
they are the labels used in planning, CI evidence, and status reviews.

| Ledger `targetPhase` | prompt.md phase | office-suite-plan.md phase | Meaning |
|---|---|---|---|
| `maintain` | (post-1.0) | — | Feature is done; keep it green |
| `shared-shell` | Phase 1 | Phase 1 | Shell, sessions, file management |
| `office-sessions` | Phase 4 | Phase 5 | Multi-document sessions + Office interoperability |
| `document-completion` | Phase 4 | Phase 2 | Writer parity completion |
| `spreadsheet-completion` | Phase 2 | Phase 3 | Calc parity completion |
| `presentation-completion` | Phase 3 | Phase 4 | Slides parity completion |
| `reliability-hardening` | Phase 1 | Phase 1/6 | Autosave, recovery, crash safety |
| `cross-platform-certification` | Phase 5 | Phase 6/7 | Perf budgets, a11y, 3-OS release certification |

## Status of the original plan (as of Sept 2026)

- prompt.md Phases 1-4: **done** (see `docs/capability-ledger.json` — all 25 entries at
  `partial` or above, most `implemented`/`verified`/`interoperable`).
- prompt.md Phase 5: **in progress** — release engineering (signing, updater wiring,
  certification) is the remaining work.
- Several prompt.md v1 non-goals were superseded by `office-suite-plan.md` and shipped:
  comments, tracked changes, compare, conditional formatting, pivots, slicers, scenarios,
  transitions + entrance/exit animations, slide masters, PPTX/DOCX/XLSX import.
  These are now covered by ledger evidence and tests.
- Remaining non-goals (unchanged): real-time collaboration, macros/VBA, cloud sync,
  LAMBDA, pivot charts.
