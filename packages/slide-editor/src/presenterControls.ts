export type PresenterNavigationState = {
  slideIndex: number;
  revealCount: number;
};

export function clampPresenterIndex(index: number, slideCount: number): number {
  if (slideCount <= 0) return 0;
  return Math.max(0, Math.min(slideCount - 1, Math.trunc(index)));
}

export function advancePresenter(
  state: PresenterNavigationState,
  slideCount: number,
  fadeCount: number,
): PresenterNavigationState {
  if (fadeCount > state.revealCount) {
    return { ...state, revealCount: state.revealCount + 1 };
  }
  if (state.slideIndex < slideCount - 1) {
    return { slideIndex: state.slideIndex + 1, revealCount: 0 };
  }
  return state;
}

export function backPresenter(
  state: PresenterNavigationState,
  fadeCount: number,
): PresenterNavigationState {
  if (state.revealCount > 0) {
    return { ...state, revealCount: state.revealCount - 1 };
  }
  if (state.slideIndex > 0) {
    return { slideIndex: state.slideIndex - 1, revealCount: fadeCount };
  }
  return state;
}

export function formatPresenterTimer(seconds: number): string {
  const safeSeconds = Math.max(0, Math.trunc(Number.isFinite(seconds) ? seconds : 0));
  const minutes = String(Math.floor(safeSeconds / 60)).padStart(2, "0");
  const remainder = String(safeSeconds % 60).padStart(2, "0");
  return `${minutes}:${remainder}`;
}
