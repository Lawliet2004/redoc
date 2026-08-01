/**
 * Release-CI-only installer size gate.
 *
 * When `apps/desktop/src-tauri/target/release/bundle/**` (or CARGO_TARGET_DIR
 * equivalent) contains installer artifacts after `tauri build`, fail if any
 * installer exceeds 55 MB. When no bundle artifacts exist (typical PR CI /
 * local quality:check without a full installer build), this gate is a no-op.
 */
import { existsSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const INSTALLER_LIMIT_BYTES = 55 * 1024 * 1024;
const INSTALLER_EXT = /\.(msi|dmg|deb|appimage|exe)$/i;
const SKIP_EXT = /\.(sig|json|blockmap)$/i;

function resolveBundleRoots(cwd) {
  const roots = [];
  const cargoTarget = process.env.CARGO_TARGET_DIR;
  if (cargoTarget) {
    roots.push(join(cargoTarget, "release", "bundle"));
  }
  roots.push(join(cwd, "apps", "desktop", "src-tauri", "target", "release", "bundle"));
  roots.push(join(cwd, "target", "release", "bundle"));
  return [...new Set(roots)];
}

function collectFiles(directory, out = []) {
  if (!existsSync(directory)) return out;
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) collectFiles(path, out);
    else out.push(path);
  }
  return out;
}

export function runInstallerSizeGate(cwd = process.cwd(), { log = console } = {}) {
  const installers = [];
  for (const root of resolveBundleRoots(cwd)) {
    for (const file of collectFiles(root)) {
      if (SKIP_EXT.test(file)) continue;
      if (!INSTALLER_EXT.test(file)) continue;
      installers.push(file);
    }
  }

  if (installers.length === 0) {
    log.log(
      "Installer size gate: no release bundle installers found (skipped; release-CI-only when artifacts exist).",
    );
    return { skipped: true, failures: [] };
  }

  const failures = [];
  for (const file of installers) {
    const size = statSync(file).size;
    const rel = relative(cwd, file);
    if (size > INSTALLER_LIMIT_BYTES) {
      failures.push({ file: rel, size });
      log.error(
        `Installer size gate: ${rel} is ${size} bytes (limit ${INSTALLER_LIMIT_BYTES} / 55 MB).`,
      );
    } else {
      log.log(`Installer size gate: ${rel} ok (${size} bytes).`);
    }
  }

  return { skipped: false, failures };
}

const isMain =
  process.argv[1] &&
  fileURLToPath(import.meta.url).toLowerCase() === String(process.argv[1]).toLowerCase();
if (isMain) {
  const result = runInstallerSizeGate();
  if (result.failures.length) process.exit(1);
}
