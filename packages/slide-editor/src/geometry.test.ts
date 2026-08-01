import { describe, expect, it } from "vitest";
import { snapElementPosition, computeBounds, hitTest } from "./geometry";

describe("snapElementPosition", () => {
  it("snaps an element center to the slide center", () => {
    const result = snapElementPosition(
      { id: "selected", x: 420, y: 90, width: 100, height: 60 },
      431,
      90,
      [],
    );

    expect(result.x).toBe(430);
    expect(result.guides).toContainEqual({ axis: "x", position: 480 });
  });

  it("snaps to another element edge", () => {
    const result = snapElementPosition(
      { id: "selected", x: 100, y: 100, width: 80, height: 60 },
      216,
      100,
      [{ id: "peer", x: 300, y: 100, width: 120, height: 80 }],
    );

    expect(result.x).toBe(220);
    expect(result.guides).toContainEqual({ axis: "x", position: 300 });
  });
});

describe("computeBounds", () => {
  it("computes bounds for multiple elements", () => {
    const bounds = computeBounds([
      { id: "1", x: 10, y: 10, width: 20, height: 20 },
      { id: "2", x: 50, y: 50, width: 30, height: 30 },
    ]);
    expect(bounds).toEqual({ x: 10, y: 10, width: 70, height: 70 });
  });

  it("returns null for empty array", () => {
    expect(computeBounds([])).toBeNull();
  });
});

describe("hitTest", () => {
  it("returns true when point is inside element", () => {
    const element = { id: "1", x: 10, y: 10, width: 50, height: 50 };
    expect(hitTest(element, 20, 20)).toBe(true);
    expect(hitTest(element, 10, 10)).toBe(true);
    expect(hitTest(element, 60, 60)).toBe(true);
  });

  it("returns false when point is outside element", () => {
    const element = { id: "1", x: 10, y: 10, width: 50, height: 50 };
    expect(hitTest(element, 5, 5)).toBe(false);
    expect(hitTest(element, 61, 61)).toBe(false);
    expect(hitTest(element, 20, 70)).toBe(false);
  });
});
