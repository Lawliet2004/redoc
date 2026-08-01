# Contributing to Redoc

Redoc is a pnpm workspace paired with a Cargo workspace. The frontend is SolidJS + TypeScript and the desktop backend is Tauri v2 + Rust. Git is initialized in this repository — use normal branches and pull requests.

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

## Before opening a pull request

Run the full quality bar:

```powershell
pnpm generate:bindings
pnpm lint
pnpm typecheck
pnpm test
pnpm build
pnpm quality:check
pnpm a11y:check
```

Regenerate bindings whenever Tauri command signatures change (`pnpm generate:bindings`). `quality:check` fails if `packages/api-client/src/generated.ts` is missing or stale relative to the Rust command list.

Also walk through the manual steps in [docs/qa-checklist.md](docs/qa-checklist.md) for areas you touched.

**Do not commit** one-off scratch scripts, local experiment files, or `.env` secrets. Keep commits focused on the change under review.
