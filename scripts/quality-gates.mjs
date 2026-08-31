import { spawnSync } from "node:child_process";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { gzipSync } from "node:zlib";
import { join, relative } from "node:path";
import { runInstallerSizeGate } from "./installer-size-gate.mjs";
import { runPerfBudgetGate } from "./perf-budget.mjs";

const dist = join(process.cwd(), "apps", "desktop", "dist");
if (!existsSync(dist)) {
  console.error("Quality gate: apps/desktop/dist is missing; run pnpm build first.");
  process.exit(1);
}

const files = [];
const visit = (directory) => {
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) visit(path);
    else files.push(path);
  }
};
visit(dist);

const frontendBytes = files
  .filter((file) => /\.(js|css|html)$/.test(file))
  .reduce((total, file) => total + gzipSync(readFileSync(file)).length, 0);
const frontendLimit = 2.2 * 1024 * 1024;
if (frontendBytes > frontendLimit) {
  console.error(`Quality gate: gzipped frontend is ${frontendBytes} bytes (limit ${frontendLimit}).`);
  process.exit(1);
}

const html = files.filter((file) => file.endsWith("index.html")).map((file) => readFileSync(file, "utf8")).join("\n");
for (const required of ["<title>", "name=\"viewport\""]) {
  if (!html.includes(required)) {
    console.error(`Quality gate: built HTML is missing ${required}.`);
    process.exit(1);
  }
}

const generatedBindings = join(process.cwd(), "packages", "api-client", "src", "generated.ts");
if (!existsSync(generatedBindings)) {
  console.error("Quality gate: generated API bindings are missing; run pnpm generate:bindings.");
  process.exit(1);
}
const generatedSource = readFileSync(generatedBindings, "utf8");
if (!generatedSource.includes("tauri-specta") || !generatedSource.includes("export const commands")) {
  console.error("Quality gate: generated API bindings do not contain the tauri-specta marker and command registry.");
  process.exit(1);
}

const formulaCatalog = spawnSync("node", ["scripts/generate-formula-catalog.mjs", "--check"], {
  cwd: process.cwd(),
  encoding: "utf8",
  shell: true,
});
if (formulaCatalog.status !== 0) {
  console.error("Quality gate: generated formula catalog is stale.");
  if (formulaCatalog.stderr?.trim()) console.error(formulaCatalog.stderr.trim());
  process.exit(formulaCatalog.status ?? 1);
}

const sourceRoot = join(process.cwd(), "packages");
const sourceFiles = [];
const visitSource = (directory) => {
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) visitSource(path);
    else if (/\.(ts|tsx)$/.test(path)) sourceFiles.push(path);
  }
};
visitSource(sourceRoot);
const directInvokeUsers = sourceFiles.filter((file) => {
  if (file === generatedBindings) return false;
  return readFileSync(file, "utf8").includes("@tauri-apps/api/core");
});
if (directInvokeUsers.length) {
  console.error(
    `Quality gate: direct Tauri API imports must stay in generated bindings: ${directInvokeUsers
      .map((file) => relative(process.cwd(), file))
      .join(", ")}`,
  );
  process.exit(1);
}

const tauriConfigPath = join(process.cwd(), "apps", "desktop", "src-tauri", "tauri.conf.json");
const tauriConfig = JSON.parse(readFileSync(tauriConfigPath, "utf8"));
if (tauriConfig.mainBinaryName !== "redoc-desktop-tauri") {
  console.error("Quality gate: Tauri mainBinaryName must target redoc-desktop-tauri.");
  process.exit(1);
}

const accessibilityMarkers = [
  [join(process.cwd(), "packages", "ui", "src", "SegmentedControl.tsx"), ["role=\"group\"", "aria-pressed"]],
  [join(process.cwd(), "packages", "sheet-editor", "src", "SheetEditor.tsx"), ["aria-label=\"Formula input\"", "aria-label=\"Spreadsheet grid\"", "aria-rowcount=\"100000\"", "aria-live=\"polite\""]],
  [join(process.cwd(), "packages", "slide-editor", "src", "SlideEditor.tsx"), ["aria-label=\"Slide canvas\""]],
  [join(process.cwd(), "packages", "ui", "src", "Dialog.tsx"), ["aria-modal=\"true\"", "focusableSelector"]],
  [join(process.cwd(), "packages", "editor-common", "src", "CommandPalette.tsx"), ["aria-label=\"Command search\""]],
  [join(process.cwd(), "packages", "editor-common", "src", "ContextMenu.tsx"), ["role=\"menu\"", "aria-label=\"Context menu\""]],
  [join(process.cwd(), "packages", "editor-common", "src", "FindBar.tsx"), ["aria-live=\"polite\""]],
  [join(process.cwd(), "packages", "editor-common", "src", "Toolbar.tsx"), ["ArrowLeft", "ArrowRight", "g-toolbar-btn", "tabIndex"]],
  [join(process.cwd(), "packages", "ui", "src", "theme.css"), ["prefers-contrast: more"]],
  [join(process.cwd(), "apps", "desktop", "src", "App.tsx"), ["aria-live=\"polite\"", "redoc-open-file"]],
];
for (const [file, markers] of accessibilityMarkers) {
  const source = readFileSync(file, "utf8");
  const missing = markers.filter((marker) => !source.includes(marker));
  if (missing.length) {
    console.error(`Quality gate: ${relative(process.cwd(), file)} is missing accessibility markers: ${missing.join(", ")}.`);
    process.exit(1);
  }
}

const a11y = spawnSync("pnpm", ["a11y:check"], {
  cwd: process.cwd(),
  stdio: "inherit",
  shell: true,
});
if (a11y.status !== 0) {
  console.error("Quality gate: axe-core a11y:check failed.");
  process.exit(a11y.status ?? 1);
}

const installerGate = runInstallerSizeGate(process.cwd());
if (installerGate.failures.length) {
  process.exit(1);
}

// Always verify the warm-recalc budget is encoded in the bench (lightweight).
const benchSource = readFileSync(
  join(process.cwd(), "crates", "sheet-engine", "benches", "grid_render.rs"),
  "utf8",
);
if (!benchSource.includes("WARM_LIMIT_MS") || !benchSource.includes("warm_median")) {
  console.error("Quality gate: grid_render bench must enforce warm_median / WARM_LIMIT_MS.");
  process.exit(1);
}

// Full cargo bench is heavier; run on release CI or when explicitly requested.
if (process.env.REDOC_PERF_GATE === "1" || process.env.REDOC_RELEASE_CI === "1") {
  const perfGate = runPerfBudgetGate();
  if (perfGate.failed) {
    process.exit(1);
  }
} else {
  console.log(
    "Perf budget gate: source contract ok; set REDOC_PERF_GATE=1 or REDOC_RELEASE_CI=1 to run cargo bench.",
  );
}

console.log(`Quality gates passed: ${files.length} files, ${frontendBytes} gzipped frontend bytes.`);
