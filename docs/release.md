# Redoc release guide

1. Run `pnpm lint`, `pnpm typecheck`, `pnpm test`, `pnpm build`, and `pnpm quality:check` on the release commit. The quality gate enforces the 2.2 MB gzipped frontend budget, axe-core a11y checks, basic built-HTML accessibility prerequisites, and—**when installer artifacts exist**—the 55 MB installer size gate (release-CI-only; skipped when `target/release/bundle/**` is empty). Warm-recalc performance budget (≤16 ms median) runs when `REDOC_PERF_GATE=1` or `REDOC_RELEASE_CI=1`. Tag release CI sets `REDOC_RELEASE_CI=1` and re-runs `node scripts/installer-size-gate.mjs` after `tauri build`.
2. Execute `docs/qa-checklist.md` on Windows, macOS, and Linux build environments.
3. Push a `v*` tag to run `.github/workflows/release.yml`. It builds Linux AppImage/deb, Windows NSIS/MSI, and macOS DMG artifacts and creates a draft prerelease after repeating the validation gates.
4. Build installers locally with `pnpm tauri build` after installing the platform-specific Tauri prerequisites.
5. Inspect installer size (`scripts/installer-size-gate.mjs` / `pnpm quality:check` after a release bundle) and verify that `.redoc` files open from the installed application into the correct editor mode. Bundle `fileAssociations` for `.redoc` are declared in `apps/desktop/src-tauri/tauri.conf.json` (Windows NSIS/MSI, macOS CFBundleDocumentTypes, Linux MIME via Tauri packaging). OS open is handled via `RunEvent::Opened` (macOS) and argv paths (Windows/Linux), emitted to the frontend as `redoc-open-file`.
6. Configure signing and updater secrets (see below). The tag workflow generates a temporary updater config and enables the `updater` Cargo feature only when the public-key and endpoint secrets are present; otherwise it produces unsigned early-release bundles.
7. Publish a signed update manifest only after smoke testing install, upgrade, and rollback behavior.

## Code signing and updater environment variables

| Variable | Purpose |
| --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY` | Minisign private key used by Tauri to sign updater artifacts |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Password for the signing private key (if encrypted) |
| `TAURI_UPDATER_PUBLIC_KEY` | Public key embedded in release updater config for client verification |
| `TAURI_UPDATER_ENDPOINT` | HTTPS endpoint that serves the update JSON manifest |

### Windows (Authenticode / store packaging)

| Variable | Purpose |
| --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY` / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Updater artifact signing (required for signed updates) |
| `WINDOWS_CERTIFICATE` | Base64-encoded `.pfx` / code-signing certificate (optional; omit for unsigned early releases) |
| `WINDOWS_CERTIFICATE_PASSWORD` | Password for `WINDOWS_CERTIFICATE` |
| `TAURI_WINDOWS_SIGNTOOL_PATH` | Optional path to `signtool.exe` when not on `PATH` |

### macOS (Developer ID / notarization)

| Variable | Purpose |
| --- | --- |
| `APPLE_CERTIFICATE` | Base64-encoded Developer ID Application certificate (`.p12`) |
| `APPLE_CERTIFICATE_PASSWORD` | Password for the `.p12` |
| `APPLE_SIGNING_IDENTITY` | Signing identity string (e.g. `Developer ID Application: …`) |
| `APPLE_ID` / `APPLE_PASSWORD` / `APPLE_TEAM_ID` | Notarization Apple ID credentials (app-specific password) |
| `TAURI_SIGNING_PRIVATE_KEY` / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Updater artifact signing |

Unsigned local and early CI builds are supported: omit platform cert secrets and updater secrets. Settings → **Check for updates** calls the updater plugin when the `updater` feature is compiled; missing certs or plugin registration must not crash the app.

The v1 updater is intentionally release-only. Development builds do not contact an update service.

The updater feature is compiled only when `--features updater` is passed to the Tauri build. The release workflow supplies that flag and `tauri.generated.release.conf.json` only when both updater secrets are available; the generated config is ignored and must never be committed.
