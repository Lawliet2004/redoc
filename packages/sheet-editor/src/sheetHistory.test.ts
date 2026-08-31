import { describe, expect, it } from "vitest";
import { createSheetHistoryHandlers } from "./sheetModel";
import type { GridCell, SheetSnapshot } from "./sheetTypes";

function makeCell(raw: string): GridCell {
  return { raw, display: raw };
}

function emptySnapshot(cells: Record<string, GridCell> = {}): SheetSnapshot {
  return {
    cells,
    merges: [],
    autoFilterEnabled: false,
    filterRange: null,
    columnFilters: {},
    columnWidths: {},
    rowHeights: {},
    chartType: null,
    chartTitle: "",
    chartRange: { startRow: 0, endRow: 0, startCol: 0, endCol: 0 },
    conditionalFormatting: [],
    sheetsMeta: [{ id: "sheet-1", name: "Sheet1" }],
    activeSheetIndex: 0,
  };
}

function createHistoryHarness(initialCells: Record<string, GridCell> = {}) {
  let cells = { ...initialCells };
  let past: SheetSnapshot[] = [];
  let future: SheetSnapshot[] = [];

  const captureSnapshot = () => emptySnapshot({ ...cells });
  const restoreSnapshot = async (snap: SheetSnapshot) => {
    cells = { ...snap.cells };
  };

  const { pushHistory, undo, redo } = createSheetHistoryHandlers({
    historyPast: () => past,
    setHistoryPast: (value) => {
      past = typeof value === "function" ? value(past) : value;
    },
    historyFuture: () => future,
    setHistoryFuture: (value) => {
      future = typeof value === "function" ? value(future) : value;
    },
    captureSnapshot,
    restoreSnapshot,
  });

  return {
    get cells() {
      return cells;
    },
    setCells(next: Record<string, GridCell>) {
      cells = next;
    },
    get past() {
      return past;
    },
    get future() {
      return future;
    },
    pushHistory,
    undo,
    redo,
  };
}

describe("createSheetHistoryHandlers", () => {
  it("pushHistory captures a snapshot on the past stack", () => {
    const harness = createHistoryHarness({ "0,0": makeCell("A") });
    harness.pushHistory();
    harness.setCells({ "0,0": makeCell("B") });

    expect(harness.past).toHaveLength(1);
    expect(harness.past[0].cells["0,0"].raw).toBe("A");
    expect(harness.cells["0,0"].raw).toBe("B");
  });

  it("undo restores the previous cell snapshot and moves current to future", async () => {
    const harness = createHistoryHarness({ "0,0": makeCell("A") });
    harness.pushHistory();
    harness.setCells({ "0,0": makeCell("B") });

    await harness.undo();

    expect(harness.cells["0,0"].raw).toBe("A");
    expect(harness.past).toHaveLength(0);
    expect(harness.future).toHaveLength(1);
    expect(harness.future[0].cells["0,0"].raw).toBe("B");
  });

  it("redo restores a future snapshot", async () => {
    const harness = createHistoryHarness({ "0,0": makeCell("A") });
    harness.pushHistory();
    harness.setCells({ "0,0": makeCell("B") });
    await harness.undo();

    await harness.redo();

    expect(harness.cells["0,0"].raw).toBe("B");
    expect(harness.future).toHaveLength(0);
    expect(harness.past).toHaveLength(1);
  });

  it("pushHistory clears the future stack", async () => {
    const harness = createHistoryHarness({ "0,0": makeCell("A") });
    harness.pushHistory();
    harness.setCells({ "0,0": makeCell("B") });
    await harness.undo();
    expect(harness.future).toHaveLength(1);

    harness.pushHistory();
    expect(harness.future).toHaveLength(0);
  });

  it("caps history stacks at 200 entries", () => {
    const harness = createHistoryHarness();
    for (let i = 0; i < 205; i++) {
      harness.setCells({ "0,0": makeCell(String(i)) });
      harness.pushHistory();
    }
    expect(harness.past).toHaveLength(200);
    expect(harness.past[0].cells["0,0"].raw).toBe("5");
    expect(harness.past[199].cells["0,0"].raw).toBe("204");
  });
});
