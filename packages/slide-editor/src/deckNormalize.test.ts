import { describe, expect, it } from "vitest";
import { defaultTheme, normalizeDeck, normalizeSlideComments, normalizeTransition, toDeck } from "./deckNormalize";

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
});
