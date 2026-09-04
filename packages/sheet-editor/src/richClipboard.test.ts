import { beforeEach, describe, expect, it } from "vitest";
import {
  peekRichClipboard,
  setRichClipboard,
  shiftFormulaReferences,
  takeRichClipboard,
} from "./richClipboard";

describe("rich clipboard", () => {
  beforeEach(() => {
    setRichClipboard(null);
  });

  it("keeps a copy stash reusable and makes a cut stash single-use", () => {
    setRichClipboard({
      kind: "sheet",
      startRow: 1,
      startCol: 1,
      endRow: 2,
      endCol: 2,
      cells: {},
      cut: false,
    });
    expect(takeRichClipboard()).not.toBeNull();
    expect(peekRichClipboard()).not.toBeNull();

    setRichClipboard({
      kind: "sheet",
      startRow: 1,
      startCol: 1,
      endRow: 2,
      endCol: 2,
      cells: {},
      cut: true,
    });
    expect(takeRichClipboard()).not.toBeNull();
    expect(takeRichClipboard()).toBeNull();
  });

  describe("shiftFormulaReferences", () => {
    it("shifts relative references and leaves absolute ones alone", () => {
      expect(shiftFormulaReferences("=A1+B2", 1, 1)).toBe("=B2+C3");
      expect(shiftFormulaReferences("=$A$1+$B2+C$3", 2, 2)).toBe("=$A$1+$B4+E$3");
      expect(shiftFormulaReferences("=SUM(A1:A5)", 3, 0)).toBe("=SUM(A4:A8)");
    });

    it("clamps to row/column 1 instead of wrapping", () => {
      expect(shiftFormulaReferences("=A1", -5, -5)).toBe("=A1");
      expect(shiftFormulaReferences("=B2", -1, -1)).toBe("=A1");
    });

    it("does not touch text inside string literals", () => {
      expect(shiftFormulaReferences('="A1" + A1', 1, 0)).toBe('="A1" + A2');
      expect(shiftFormulaReferences('=CONCAT("Q", "R1")', 2, 2)).toBe('=CONCAT("Q", "R1")');
    });

    it("handles multi-letter columns", () => {
      expect(shiftFormulaReferences("=AA1+AB10", 0, 1)).toBe("=AB1+AC10");
      expect(shiftFormulaReferences("=AZ1", 0, 1)).toBe("=BA1");
    });

    it("leaves non-formulas untouched", () => {
      expect(shiftFormulaReferences("hello", 1, 1)).toBe("hello");
      expect(shiftFormulaReferences("A1", 1, 1)).toBe("A1");
    });

    it("handles underscores and function names without corrupting them", () => {
      expect(shiftFormulaReferences("=IF(A1>1, SUM(B1:B2), 0)", 1, 0)).toBe("=IF(A2>1, SUM(B2:B3), 0)");
    });
  });
});
