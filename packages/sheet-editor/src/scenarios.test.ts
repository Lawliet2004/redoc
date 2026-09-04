import { describe, expect, it } from "vitest";
import { applyScenario, captureScenario, MAX_SCENARIO_CELLS } from "./scenarios";

describe("what-if scenarios", () => {
  it("captures a rectangular range including blank overrides", () => {
    const scenario = captureScenario(
      { "1:1": { raw: "Price", display: "Price" }, "2:1": { raw: "12", display: "12" } },
      { startRow: 1, endRow: 2, startCol: 1, endCol: 2 },
      "s1",
      "Higher prices",
    );
    expect(scenario).toEqual({
      id: "s1",
      name: "Higher prices",
      changes: [
        { row: 1, col: 1, rawValue: "Price" },
        { row: 1, col: 2, rawValue: "" },
        { row: 2, col: 1, rawValue: "12" },
        { row: 2, col: 2, rawValue: "" },
      ],
    });
  });

  it("applies overrides immutably and removes blanked cells", () => {
    const source = {
      "1:1": { raw: "10", display: "10", style: { bold: true } },
      "1:2": { raw: "keep", display: "keep" },
    };
    const next = applyScenario(source, {
      id: "s1",
      name: "Scenario",
      changes: [
        { row: 1, col: 1, rawValue: "25" },
        { row: 1, col: 2, rawValue: "" },
        { row: 2, col: 1, rawValue: "=A1*2" },
      ],
    });
    expect(next["1:1"]).toMatchObject({ raw: "25", display: "25", style: { bold: true } });
    expect(next["1:2"]).toBeUndefined();
    expect(next["2:1"]).toMatchObject({ raw: "=A1*2", display: "=A1*2" });
    expect(source["1:1"].raw).toBe("10");
  });

  it("rejects oversized capture and apply inputs", () => {
    expect(() => captureScenario(
      {},
      { startRow: 1, endRow: MAX_SCENARIO_CELLS + 1, startCol: 1, endCol: 1 },
      "large",
      "Large",
    )).toThrow(/limited/);
    expect(() => applyScenario({}, {
      id: "large",
      name: "Large",
      changes: Array.from({ length: MAX_SCENARIO_CELLS + 1 }, (_, index) => ({ row: index, col: 1, rawValue: "1" })),
    })).toThrow(/limited/);
  });
});
