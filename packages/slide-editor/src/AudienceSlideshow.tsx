import { createSignal, onCleanup, onMount, Show } from "solid-js";
import { SlideStage, normalizePresenterSlides, type PresenterSlide } from "./PresenterView";
import { clampPresenterIndex, advancePresenter, backPresenter } from "./presenterControls";
import "./SlideEditor.css";

export type SlideshowExitReason = "escape" | "end";

const TRANSITION_CLASS: Record<string, string> = {
  "fade": "g-slide-anim-fade",
  "slide-left": "g-slide-anim-left",
  "slide-right": "g-slide-anim-right",
  "wipe-left": "g-slide-anim-wipe-left",
  "wipe-right": "g-slide-anim-wipe-right",
  "zoom": "g-slide-anim-zoom",
  "dissolve": "g-slide-anim-dissolve",
  "morph": "g-slide-anim-morph",
};

function transitionClass(transition: string | undefined): string {
  if (!transition || transition === "none") return "";
  return TRANSITION_CLASS[transition] || "";
}

/**
 * Fullscreen in-window audience slideshow (PowerPoint's F5 scenario).
 * Renders the same presenter scene at fit-to-screen scale, advancing on
 * arrow keys / Space / click; Esc exits. Key handling runs on a
 * capture-phase listener with preventDefault so editor shortcuts never
 * fire while presenting.
 */
export function AudienceSlideshow(props: {
  deck: unknown;
  startIndex: number;
  onExit: (reason: SlideshowExitReason) => void;
}) {
  const slides = (): PresenterSlide[] => normalizePresenterSlides(props.deck);
  const canvasWidth = () => {
    const deck = props.deck as any;
    return Number(deck?.canvasWidth) || 960;
  };
  const canvasHeight = () => {
    const deck = props.deck as any;
    return Number(deck?.canvasHeight) || 540;
  };
  const theme = () => (props.deck as any)?.theme;
  const masters = () => (props.deck as any)?.masters;

  const [index, setIndex] = createSignal(clampPresenterIndex(props.startIndex, slides().length));
  const [animClass, setAnimClass] = createSignal("");
  const [revealCount, setRevealCount] = createSignal(0);
  const [scale, setScale] = createSignal(1);
  let containerEl: HTMLDivElement | undefined;
  let animTimer: number | undefined;

  const current = () => slides()[index()] || slides()[0];

  const fadeCount = () => current().elements.filter((e) => e.entrance !== "none").length;

  const applyTransition = (transition: string | undefined) => {
    const next = transitionClass(transition);
    setAnimClass(next);
    if (next) {
      if (animTimer !== undefined) window.clearTimeout(animTimer);
      animTimer = window.setTimeout(() => {
        animTimer = undefined;
        setAnimClass("");
      }, transition === "dissolve" ? 550 : 450);
    }
  };

  const goTo = (idx: number) => {
    const clamped = clampPresenterIndex(idx, slides().length);
    if (clamped === index()) return;
    setIndex(clamped);
    setRevealCount(0);
    applyTransition(slides()[clamped]?.transition);
  };

  const advance = () => {
    // Step entrance animations first (matching the presenter window), then
    // advance slides — F5 playback previously skipped reveals entirely.
    const next = advancePresenter(
      { slideIndex: index(), revealCount: revealCount() },
      slides().length,
      fadeCount(),
    );
    if (next.slideIndex === slides().length - 1 && next.slideIndex === index() && next.revealCount === revealCount()) {
      // No movement possible: last slide fully revealed -> exit like before.
      props.onExit("end");
      return;
    }
    if (next.slideIndex !== index()) {
      setIndex(next.slideIndex);
      setRevealCount(0);
      applyTransition(slides()[next.slideIndex]?.transition);
    } else {
      setRevealCount(next.revealCount);
    }
  };
  const back = () => {
    const next = backPresenter(
      { slideIndex: index(), revealCount: revealCount() },
      fadeCount(),
    );
    if (next.slideIndex !== index()) {
      setIndex(next.slideIndex);
      applyTransition(slides()[next.slideIndex]?.transition);
    }
    setRevealCount(next.revealCount);
  };

  const recomputeScale = () => {
    const w = canvasWidth();
    const h = canvasHeight();
    const next = Math.min(
      (window.innerWidth - 24) / w,
      (window.innerHeight - 24) / h,
    );
    setScale(Number.isFinite(next) && next > 0 ? next : 1);
  };

  const onKey = (event: KeyboardEvent) => {
    if (event.key === "ArrowRight" || event.key === " " || event.key === "PageDown") {
      event.preventDefault();
      event.stopPropagation();
      advance();
    } else if (event.key === "ArrowLeft" || event.key === "PageUp") {
      event.preventDefault();
      event.stopPropagation();
      back();
    } else if (event.key === "Home") {
      event.preventDefault();
      event.stopPropagation();
      goTo(0);
    } else if (event.key === "End") {
      event.preventDefault();
      event.stopPropagation();
      goTo(slides().length - 1);
    } else if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      props.onExit("escape");
    }
  };

  onMount(() => {
    recomputeScale();
    window.addEventListener("resize", recomputeScale);
    window.addEventListener("keydown", onKey, true);
    containerEl?.requestFullscreen?.().catch(() => undefined);
    applyTransition(current()?.transition);
    onCleanup(() => {
      window.removeEventListener("resize", recomputeScale);
      window.removeEventListener("keydown", onKey, true);
      if (animTimer !== undefined) window.clearTimeout(animTimer);
      if (document.fullscreenElement) document.exitFullscreen?.().catch(() => undefined);
    });
  });

  return (
    <div
      ref={(el) => { containerEl = el; }}
      data-fullscreen-overlay="slide-show"
      style={{
        position: "fixed",
        inset: "0",
        "z-index": "9999",
        background: "#000",
        display: "flex",
        "align-items": "center",
        "justify-content": "center",
        cursor: "pointer",
      }}
      onClick={() => advance()}
    >
      <Show when={current()}>
        <SlideStage
          slide={current()}
          theme={theme()}
          revealCount={revealCount()}
          animClass={animClass()}
          scale={scale()}
          canvasWidth={canvasWidth()}
          canvasHeight={canvasHeight()}
          masters={masters()}
          slideIndex={index()}
        />
      </Show>
      <div class="slide-show-counter">
        {index() + 1} / {slides().length}
      </div>
    </div>
  );
}
