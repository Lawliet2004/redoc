import { describe, expect, it } from "vitest";
import { defaultTheme, normalizeDeck, toDeck } from "./deckNormalize";

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
