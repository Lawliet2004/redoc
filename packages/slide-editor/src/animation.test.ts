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
});
