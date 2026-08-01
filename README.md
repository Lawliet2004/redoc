# Redoc

Offline-first desktop office suite (Documents, Spreadsheets, Presentations) with a Google Workspace–style UI. Built with Tauri v2, Rust, SolidJS, and ProseMirror.

## Prerequisites (Windows)

- Node.js 20+
- pnpm 9+ (`npm i -g pnpm`)
- Rust stable (`rustup`)
- [Tauri v2 Windows prerequisites](https://v2.tauri.app/start/prerequisites/) (WebView2 is usually already installed on Windows 10/11)
- Visual Studio Build Tools with C++ workload (for compiling Tauri)

## Install

```powershell
cd C:\Users\Papan Ghosh\Desktop\Projects\redoc
pnpm install
```

## Run (development)

Frontend-only (Vite, no native dialogs/save):

```powershell
pnpm dev
```

Full desktop app (Vite + Tauri):

```powershell
pnpm tauri:dev
```

Or:

```powershell
pnpm tauri dev
```

## Build

```powershell
pnpm build
pnpm tauri build
```

## Test & quality

```powershell
pnpm typecheck
pnpm test
pnpm lint
```

## Layout

- `apps/desktop` — SolidJS shell + Tauri v2 host
- `crates/*` — Rust core, file-io (`.redoc`), formula, engines, export
- `packages/*` — UI, editors (doc/sheet/slide), api-client, icons

Native files use the ZIP+JSON `.redoc` container. All filesystem access goes through Tauri commands.
