import { describe, expect, it } from "vitest";
import { defaultTheme, normalizeDeck, toDeck } from "./deckNormalize";

describe("slide entrance animation fidelity", () => {
  it("preserves timing and order through kind-based deck round trips", () => {
    const slides = normalizeDeck({
      slides: [{
        id: "s1",
        layout: "blank",
        elements: [{
          id: "a",
          x: 10,
          y: 20,
          width: 200,
          height: 60,
          entrance: "fade",
          entranceDelayMs: 120,
          entranceDurationMs: 900,
          entranceOrder: 2,
          kind: { Text: { text: "Hello" } },
        }],
      }],
    });

    expect(slides[0].elements[0]).toMatchObject({
      entrance: "fade",
      entranceDelayMs: 120,
      entranceDurationMs: 900,
      entranceOrder: 2,
    });

    const persisted = toDeck(slides, {}, defaultTheme, 0);
    const restored = normalizeDeck(persisted);
    expect(restored[0].elements[0]).toMatchObject({
      entrance: "fade",
      entranceDelayMs: 120,
      entranceDurationMs: 900,
      entranceOrder: 2,
    });
  });

  it("clamps unsafe timing values and assigns deterministic fallback order", () => {
    const slides = normalizeDeck({
      slides: [{
        id: "s1",
        layout: "blank",
        elements: [
          { id: "a", type: "text", content: "A", entrance: "fade", entranceDelayMs: -4, entranceDurationMs: 1 },
          { id: "b", type: "text", content: "B", entrance: "fade", entranceDelayMs: Number.POSITIVE_INFINITY },
        ],
      }],
    });

    expect(slides[0].elements[0]).toMatchObject({ entranceDelayMs: 0, entranceDurationMs: 50, entranceOrder: 0 });
    expect(slides[0].elements[1]).toMatchObject({ entranceDelayMs: 0, entranceDurationMs: 350, entranceOrder: 1 });
  });

  it("preserves the supported zoom entrance effect through normalization", () => {
    const slides = normalizeDeck({
      slides: [{
        id: "s1",
        layout: "blank",
        elements: [{ id: "a", type: "text", content: "A", entrance: "zoom" }],
      }],
    });

    expect(slides[0].elements[0].entrance).toBe("zoom");
    const persisted = toDeck(slides, {}, defaultTheme, 0);
    expect(persisted.slides[0].elements[0].entrance).toBe("zoom");
  });

  it("preserves fade exit effects and clamps unsafe exit durations", () => {
    const slides = normalizeDeck({
      slides: [{
        id: "s1",
        layout: "blank",
        elements: [{ id: "a", type: "text", content: "A", exit: "fade", exitDurationMs: -5 }],
      }],
    });

    expect(slides[0].elements[0].exit).toBe("fade");
    expect(slides[0].elements[0].exitDurationMs).toBe(50);

    const persisted = toDeck(slides, {}, defaultTheme, 0);
    expect(persisted.slides[0].elements[0].exit).toBe("fade");
    expect(persisted.slides[0].elements[0].exitDurationMs).toBe(50);
    const restored = normalizeDeck(persisted);
    expect(restored[0].elements[0].exit).toBe("fade");
    expect(restored[0].elements[0].exitDurationMs).toBe(50);
  });

  it("normalizes unsupported exit kinds to none", () => {
    const slides = normalizeDeck({
      slides: [{
        id: "s1",
        layout: "blank",
        elements: [{ id: "a", type: "text", content: "A", exit: "zoom" }],
      }],
    });
    expect(slides[0].elements[0].exit).toBe("none");
  });
});
