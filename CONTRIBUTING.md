# Contributing to Redoc

Redoc is a pnpm workspace paired with a Cargo workspace. The frontend is SolidJS + TypeScript and the desktop backend is Tauri v2 + Rust.

## Prerequisites

- Node.js 20 or newer
- pnpm 9 or newer
- Rust stable and the Tauri v2 system dependencies for your OS

## Local workflow

```powershell
pnpm install
pnpm dev
pnpm typecheck
pnpm test
pnpm build
```

Frontend code talks to the filesystem only through `packages/api-client`. Rust domain models own persisted data and must remain independent of UI packages. New dependencies belong in `crates/README-deps.md` with a size and alternatives note.

Before opening a pull request, run `pnpm lint`, `pnpm typecheck`, `pnpm test`, and the relevant manual checks in `docs/qa-checklist.md`.
