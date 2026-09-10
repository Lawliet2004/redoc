import { describe, expect, it } from "vitest";
import { defaultMasters, defaultTheme, generateSlideId, normalizeCanvasSize, normalizeDeck, normalizeSlideComments, normalizeTransition, slideChromeOverlays, toDeck } from "./deckNormalize";

describe("normalizeDeck", () => {
  it("returns a default title slide when deck is empty", () => {
    const slides = normalizeDeck(null);
    expect(slides).toHaveLength(1);
    expect(slides[0].layout).toBe("title");
    expect(slides[0].elements).toHaveLength(2);
  });

  it("normalizes kind.Text elements from persisted deck JSON", () => {
    const slides = normalizeDeck({
      slides: [{
        id: "s1",
        layout: "blank",
        elements: [{
          id: "t1",
          x: 10,
          y: 20,
          width: 300,
          height: 80,
          kind: { Text: { text: "Hello", fontSize: 24, color: "#111", align: "left", bullets: true } },
        }],
      }],
    });

    expect(slides[0].elements[0]).toMatchObject({
      id: "t1",
      type: "text",
      content: "Hello",
      fontSize: 24,
      align: "left",
      bullets: true,
    });
  });

  it("inherits fade transition from deck-level fadeBetweenSlides", () => {
    const slides = normalizeDeck({
      fadeBetweenSlides: true,
      slides: [{ id: "s1", layout: "blank", elements: [] }],
    });
    expect(slides[0].transition).toBe("fade");
  });

  it("accepts the bounded native transition subset", () => {
    expect(normalizeTransition("wipe-left")).toBe("wipe-left");
    expect(normalizeTransition("wipe-right")).toBe("wipe-right");
    expect(normalizeTransition("zoom")).toBe("zoom");
    expect(normalizeTransition("dissolve")).toBe("dissolve");
    expect(normalizeTransition("unknown")).toBe("none");
  });

  it("normalizes slide comments and drops empty entries", () => {
    const comments = normalizeSlideComments([
      { id: "c1", author: "  Alice  ", text: "Tighten this headline", createdAt: "2026-01-01T00:00:00Z" },
      { id: "c2", author: "", text: "   " },
      null,
      42,
    ]);
    expect(comments).toHaveLength(1);
    expect(comments[0]).toMatchObject({
      id: "c1",
      author: "Alice",
      text: "Tighten this headline",
      resolved: false,
    });
    // Malformed authors fall back to "You" while the text survives.
    const salvaged = normalizeSlideComments([{ author: 42, text: "kept" }]);
    expect(salvaged).toHaveLength(1);
    expect(salvaged[0].author).toBe("You");
    expect(salvaged[0].text).toBe("kept");
    expect(normalizeSlideComments(undefined)).toEqual([]);
    expect(normalizeSlideComments("nope")).toEqual([]);
  });

  it("round-trips slide comments through toDeck", () => {
    const slides = normalizeDeck({
      slides: [{
        id: "s1",
        layout: "blank",
        elements: [],
        comments: [{ id: "c1", author: "Bob", text: "Add a chart", resolved: false, createdAt: "2026-02-02T00:00:00Z" }],
      }],
    });
    const deck = toDeck(slides, { canvasWidth: 800, canvasHeight: 450 }, defaultTheme, 0);
    expect(deck.slides[0].comments).toHaveLength(1);
    expect(deck.slides[0].comments[0].text).toBe("Add a chart");
  });
});

describe("toDeck", () => {
  it("round-trips normalized slides back to deck JSON", () => {
    const slides = normalizeDeck({
      slides: [{
        id: "s1",
        layout: "blank",
        notes: "Speaker notes",
        elements: [{
          id: "t1",
          x: 0,
          y: 0,
          width: 200,
          height: 50,
          kind: { Text: { text: "Hi", fontSize: 18, color: "#000", align: "center" } },
        }],
      }],
    });

    const deck = toDeck(slides, { canvasWidth: 800, canvasHeight: 450 }, defaultTheme, 0);
    expect(deck.canvasWidth).toBe(800);
    expect(deck.activeSlideIndex).toBe(0);
    expect(deck.slides[0].notes).toBe("Speaker notes");
    expect(deck.slides[0].elements[0].kind.Text.text).toBe("Hi");
  });

  it("round-trips line and pie chart types with data", () => {
    const slides = normalizeDeck({
      slides: [{
        id: "s1",
        layout: "blank",
        elements: [
          { id: "c1", type: "chart", x: 0, y: 0, width: 300, height: 200, content: "", chartType: "line", chartTitle: "Trend", chartData: [1, 2], chartLabels: ["a", "b"] },
          { id: "c2", type: "chart", x: 0, y: 220, width: 300, height: 200, content: "", chartType: "pie", chartTitle: "Share", chartData: [3, 1], chartLabels: ["x", "y"] },
        ],
      }],
    });

    const deck = toDeck(slides, {}, defaultTheme, 0);
    expect(deck.slides[0].elements[0].kind.Chart.chartType).toBe("line");
    expect(deck.slides[0].elements[1].kind.Chart.chartType).toBe("pie");
    const restored = normalizeDeck(deck);
    expect(restored[0].elements[0].chartType).toBe("line");
    expect(restored[0].elements[1].chartType).toBe("pie");
    expect(restored[0].elements[0].chartData).toEqual([1, 2]);
  });

  it("round-trips table merges through kind.Table", () => {
    const slides = normalizeDeck({
      slides: [{
        id: "s1",
        layout: "blank",
        elements: [{
          id: "tbl",
          type: "table",
          x: 0,
          y: 0,
          width: 300,
          height: 100,
          content: "",
          tableData: [["a", "b", "c"], ["d", "e", "f"]],
          tableMerges: [{ r: 0, c: 0, rowspan: 1, colspan: 2 }],
          tableHeaderRow: true,
          tableStyle: "banded",
        }],
      }],
    });

    const deck = toDeck(slides, {}, defaultTheme, 0);
    expect(deck.slides[0].elements[0].kind.Table.merges).toEqual([{ r: 0, c: 0, rowspan: 1, colspan: 2 }]);
    expect(deck.slides[0].elements[0].kind.Table.headerRow).toBe(true);
    expect(deck.slides[0].elements[0].kind.Table.tableStyle).toBe("banded");
    const restored = normalizeDeck(deck);
    expect(restored[0].elements[0].tableMerges).toEqual([{ r: 0, c: 0, rowspan: 1, colspan: 2 }]);
    expect(restored[0].elements[0].tableHeaderRow).toBe(true);
    expect(restored[0].elements[0].tableStyle).toBe("banded");
  });
});

describe("generateSlideId", () => {
  it("produces unique ids across rapid successive calls", () => {
    const ids = new Set(Array.from({ length: 200 }, () => generateSlideId("slide")));
    expect(ids.size).toBe(200);
    for (const id of ids) expect(id).toMatch(/^slide-/);
  });
});

describe("normalizeCanvasSize", () => {
  it("keeps valid sizes and falls back on invalid values", () => {
    expect(normalizeCanvasSize(1280, 720)).toEqual({ width: 1280, height: 720 });
    expect(normalizeCanvasSize(undefined, null)).toEqual({ width: 960, height: 540 });
    expect(normalizeCanvasSize(-5, Number.NaN)).toEqual({ width: 960, height: 540 });
    expect(normalizeCanvasSize(99999, 540)).toEqual({ width: 5120, height: 540 });
  });
});

describe("slideChromeOverlays", () => {
  it("resolves master chrome inheritance into overlay boxes", () => {
    const slide: Parameters<typeof slideChromeOverlays>[0] = {
      id: "s1",
      title: "",
      layout: "blank",
      elements: [],
      notes: "",
      transition: "none",
      masterId: "master-corporate",
    };
    const overlays = slideChromeOverlays(slide, defaultMasters, 2, 960, 540);
    const byKey = Object.fromEntries(overlays.map((o) => [o.key, o]));
    expect(byKey.header.text).toBe("Company");
    expect(byKey.footer.text).toBe("Confidential");
    expect(byKey.number.text).toBe("3");
    expect(byKey.date.text).toMatch(/^\d{4}-\d{2}-\d{2}$/);
  });

  it("emits no overlays when the master hides everything", () => {
    const slide: Parameters<typeof slideChromeOverlays>[0] = {
      id: "s1",
      title: "",
      layout: "blank",
      elements: [],
      notes: "",
      transition: "none",
      masterId: "master-default",
      showHeader: false,
      showFooter: false,
      showDate: false,
      showSlideNumber: false,
    };
    expect(slideChromeOverlays(slide, defaultMasters, 0, 960, 540)).toEqual([]);
  });
});
