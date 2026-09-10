import { describe, expect, it } from "vitest";
import { snapElementPosition, computeBounds, hitTest, alignElements, assignGroupId, clearGroupIds, visibleThumbRange } from "./geometry";

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

  it("honors a non-960x540 canvas for clamping and center guides", () => {
    const clamped = snapElementPosition(
      { id: "selected", x: 1250, y: 400, width: 100, height: 60 },
      1250,
      400,
      [],
      1280,
      720,
    );
    expect(clamped.x).toBe(1180);

    const snapped = snapElementPosition(
      { id: "selected", x: 615, y: 325, width: 100, height: 60 },
      641,
      360,
      [],
      1280,
      720,
    );
    expect(snapped.guides).toContainEqual({ axis: "x", position: 640 });
    expect(snapped.guides).toContainEqual({ axis: "y", position: 360 });
  });
});

describe("visibleThumbRange", () => {
  it("returns a window around the scroll viewport plus the active row", () => {
    expect(visibleThumbRange(0, 200, 100, 67, 4, 0)).toEqual({ first: 0, last: 7 });
    expect(visibleThumbRange(670, 200, 100, 67, 4, 50)).toEqual({ first: 6, last: 50 });
  });

  it("clamps to the available slide count", () => {
    expect(visibleThumbRange(0, 1000, 3, 67, 4, 0)).toEqual({ first: 0, last: 2 });
  });

  it("returns an empty window for zero slides", () => {
    expect(visibleThumbRange(0, 200, 0, 67, 4, 0)).toEqual({ first: 0, last: -1 });
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

describe("alignElements", () => {
  it("centers a single element on the canvas", () => {
    const [pos] = alignElements(
      [{ id: "a", x: 0, y: 0, width: 100, height: 60 }],
      "center",
    );
    expect(pos).toEqual({ id: "a", x: 430, y: 0 });
  });

  it("aligns multiple elements to the left edge of the selection bounds", () => {
    const positions = alignElements(
      [
        { id: "a", x: 50, y: 10, width: 40, height: 20 },
        { id: "b", x: 120, y: 30, width: 60, height: 30 },
      ],
      "left",
    );
    expect(positions).toEqual([
      { id: "a", x: 50, y: 10 },
      { id: "b", x: 50, y: 30 },
    ]);
  });
});

describe("group helpers", () => {
  it("assignGroupId tags selected elements", () => {
    const elements = [
      { id: "a", x: 0, y: 0, width: 10, height: 10 },
      { id: "b", x: 20, y: 0, width: 10, height: 10 },
      { id: "c", x: 40, y: 0, width: 10, height: 10 },
    ];
    const grouped = assignGroupId(elements, ["a", "b"], "grp-1");
    expect(grouped[0].groupId).toBe("grp-1");
    expect(grouped[1].groupId).toBe("grp-1");
    expect(grouped[2].groupId).toBeUndefined();
  });

  it("clearGroupIds removes group from selection and peers in same group", () => {
    const elements = [
      { id: "a", x: 0, y: 0, width: 10, height: 10, groupId: "grp-1" },
      { id: "b", x: 20, y: 0, width: 10, height: 10, groupId: "grp-1" },
      { id: "c", x: 40, y: 0, width: 10, height: 10, groupId: "grp-2" },
    ];
    const ungrouped = clearGroupIds(elements, ["a"]);
    expect(ungrouped[0].groupId).toBeUndefined();
    expect(ungrouped[1].groupId).toBeUndefined();
    expect(ungrouped[2].groupId).toBe("grp-2");
  });
});
