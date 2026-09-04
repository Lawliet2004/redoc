import { describe, expect, it } from "vitest";
import { resolveValidationOptions } from "./validationOptions";

describe("resolveValidationOptions", () => {
  const cells = {
    "1:2": { raw: "Open", display: "Open" },
    "2:2": { raw: "Closed", display: "Closed" },
    "3:2": { raw: "Open", display: "Open" },
  };

  it("prefers literal options and removes duplicates", () => {
    expect(resolveValidationOptions({ type: "list", options: ["A", "A", "B"] }, cells)).toEqual(["A", "B"]);
  });

  it("resolves a bounded same-sheet source range", () => {
    expect(resolveValidationOptions({ type: "list", options: [], formula: "=$B$1:$B$3" }, cells, [], "Sheet1"))
      .toEqual(["Open", "Closed"]);
  });

  it("resolves a named range and rejects a different sheet", () => {
    const ranges = [{ name: "Statuses", rangeStr: "$B$1:$B$2", sheet: "Sheet1" }];
    expect(resolveValidationOptions({ type: "list", options: [], formula: "Statuses" }, cells, ranges, "Sheet1"))
      .toEqual(["Open", "Closed"]);
    expect(resolveValidationOptions({ type: "list", options: [], formula: "Statuses" }, cells, ranges, "Sheet2"))
      .toEqual([]);
  });

  it("resolves bounded sources on another sheet when that sheet is available", () => {
    const otherSheet = {
      "1:1": { raw: "East", display: "East" },
      "2:1": { raw: "West", display: "West" },
    };
    expect(resolveValidationOptions(
      { type: "list", options: [], formula: "='Lookup Sheet'!$A$1:$A$2" },
      cells,
      [],
      "Orders",
      { "Lookup Sheet": otherSheet },
    )).toEqual(["East", "West"]);
  });

  it("resolves a named range scoped to another sheet when cached", () => {
    const ranges = [{ name: "Regions", rangeStr: "$A$1:$A$2", sheet: "Lookup Sheet" }];
    expect(resolveValidationOptions(
      { type: "list", options: [], formula: "Regions" },
      cells,
      ranges,
      "Orders",
      {
        "Lookup Sheet": {
          "1:1": { raw: "East", display: "East" },
          "2:1": { raw: "West", display: "West" },
        },
      },
    )).toEqual(["East", "West"]);
  });

  it("fails closed for malformed or oversized sources", () => {
    expect(resolveValidationOptions({ type: "list", options: [], formula: "not a range" }, cells)).toEqual([]);
    expect(resolveValidationOptions({ type: "list", options: [], formula: "A1:A20000" }, cells)).toEqual([]);
  });
});
