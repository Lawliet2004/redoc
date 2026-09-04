import type { GridCell, MergeRange, ScenarioConfig } from "./sheetTypes";

/** Keep scenario metadata bounded so a large selection or malformed file cannot freeze the editor. */
export const MAX_SCENARIO_CELLS = 100_000;

function assertScenarioSize(size: number) {
  if (size > MAX_SCENARIO_CELLS) {
    throw new Error(`Scenario selections are limited to ${MAX_SCENARIO_CELLS.toLocaleString()} cells.`);
  }
}

/** Capture the raw values in a selected rectangular range as a reusable scenario. */
export function captureScenario(
  cells: Record<string, GridCell>,
  range: MergeRange,
  id: string,
  name: string,
): ScenarioConfig {
  const rowCount = range.endRow - range.startRow + 1;
  const colCount = range.endCol - range.startCol + 1;
  assertScenarioSize(rowCount * colCount);
  const changes: ScenarioConfig["changes"] = [];
  for (let row = range.startRow; row <= range.endRow; row += 1) {
    for (let col = range.startCol; col <= range.endCol; col += 1) {
      changes.push({ row, col, rawValue: cells[`${row}:${col}`]?.raw || "" });
    }
  }
  return { id, name, changes };
}

/** Apply scenario overrides without mutating the source cell map. */
export function applyScenario(
  cells: Record<string, GridCell>,
  scenario: ScenarioConfig,
): Record<string, GridCell> {
  assertScenarioSize(scenario.changes.length);
  const next = { ...cells };
  for (const change of scenario.changes) {
    const key = `${change.row}:${change.col}`;
    if (!change.rawValue.trim()) {
      delete next[key];
      continue;
    }
    const current = next[key] || { raw: "", display: "" };
    next[key] = { ...current, raw: change.rawValue, display: change.rawValue };
  }
  return next;
}
