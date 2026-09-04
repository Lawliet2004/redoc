import { describe, expect, it } from "vitest";
import {
  advancePresenter,
  backPresenter,
  clampPresenterIndex,
  formatPresenterTimer,
} from "./presenterControls";

describe("presenter controls", () => {
  it("reveals ordered elements before advancing slides", () => {
    expect(advancePresenter({ slideIndex: 0, revealCount: 0 }, 2, 2))
      .toEqual({ slideIndex: 0, revealCount: 1 });
    expect(advancePresenter({ slideIndex: 0, revealCount: 2 }, 2, 2))
      .toEqual({ slideIndex: 1, revealCount: 0 });
  });

  it("backs through reveals and restores the previous slide reveals", () => {
    expect(backPresenter({ slideIndex: 1, revealCount: 2 }, 3))
      .toEqual({ slideIndex: 1, revealCount: 1 });
    expect(backPresenter({ slideIndex: 1, revealCount: 0 }, 3))
      .toEqual({ slideIndex: 0, revealCount: 3 });
  });

  it("bounds slide indices and sanitizes timer output", () => {
    expect(clampPresenterIndex(-2, 3)).toBe(0);
    expect(clampPresenterIndex(9, 3)).toBe(2);
    expect(formatPresenterTimer(125)).toBe("02:05");
    expect(formatPresenterTimer(Number.NaN)).toBe("00:00");
  });
});
