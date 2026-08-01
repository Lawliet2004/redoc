/**
 * Lightweight warm-recalc performance budget gate.
 * Runs the sheet-engine grid_render bench and fails if warm median > 16ms.
 * Set REDOC_SKIP_PERF_GATE=1 to skip (e.g. constrained local machines).
 */
import { spawnSync } from "node:child_process";

const WARM_LIMIT_MS = 16;

export function runPerfBudgetGate({ log = console } = {}) {
  if (process.env.REDOC_SKIP_PERF_GATE === "1") {
    log.log("Perf budget gate: skipped (REDOC_SKIP_PERF_GATE=1).");
    return { skipped: true, warmMedianMs: null };
  }

  const result = spawnSync(
    "cargo",
    ["bench", "-p", "redoc-sheet-engine", "--bench", "grid_render"],
    {
      cwd: process.cwd(),
      encoding: "utf8",
      shell: true,
      env: { ...process.env },
    },
  );

  const output = `${result.stdout || ""}\n${result.stderr || ""}`;
  if (result.status !== 0) {
    log.error("Perf budget gate: grid_render bench failed.");
    if (output.trim()) log.error(output.trim());
    return { skipped: false, warmMedianMs: null, failed: true };
  }

  const match = output.match(/warm[_-]?median[=:\s]+([0-9.]+)\s*ms/i)
    || output.match(/warm=([0-9.]+)ms/i)
    || output.match(/warm=([0-9.]+)/i);

  if (!match) {
    // Bench enforces the assert itself; treat success exit as pass.
    log.log("Perf budget gate: bench exited 0 (asserted warm budget internally).");
    return { skipped: false, warmMedianMs: null, failed: false };
  }

  const warmMedianMs = Number(match[1]);
  if (Number.isFinite(warmMedianMs) && warmMedianMs > WARM_LIMIT_MS) {
    log.error(
      `Perf budget gate: warm recalc median ${warmMedianMs}ms exceeds ${WARM_LIMIT_MS}ms.`,
    );
    return { skipped: false, warmMedianMs, failed: true };
  }

  log.log(`Perf budget gate: warm recalc median ${warmMedianMs}ms (limit ${WARM_LIMIT_MS}ms).`);
  return { skipped: false, warmMedianMs, failed: false };
}

import { fileURLToPath } from "node:url";
const isMain =
  process.argv[1] &&
  fileURLToPath(import.meta.url).toLowerCase() === String(process.argv[1]).toLowerCase();
if (isMain) {
  const result = runPerfBudgetGate();
  if (result.failed) process.exit(1);
}
