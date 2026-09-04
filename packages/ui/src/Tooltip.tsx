import { JSX, createSignal, onCleanup, Show } from "solid-js";

interface TooltipProps {
  content: JSX.Element;
  shortcut?: string;
  children: JSX.Element;
  position?: "top" | "bottom" | "left" | "right";
}

export function Tooltip(props: TooltipProps) {
  const [isVisible, setIsVisible] = createSignal(false);
  const [actualPos, setActualPos] = createSignal(props.position || "top");
  let timeoutId: number;
  let containerRef!: HTMLDivElement;
  let tooltipRef!: HTMLDivElement;

  const show = () => {
    clearTimeout(timeoutId);
    timeoutId = window.setTimeout(() => {
      setActualPos(props.position || "top");
      setIsVisible(true);
      // Use requestAnimationFrame to measure after it's in the DOM
      requestAnimationFrame(() => {
        if (!tooltipRef) return;
        const rect = tooltipRef.getBoundingClientRect();
        const pref = props.position || "top";
        let newPos = pref;

        if (pref === "top" && rect.top < 0) {
          newPos = "bottom";
        } else if (pref === "bottom" && rect.bottom > window.innerHeight) {
          newPos = "top";
        } else if (pref === "left" && rect.left < 0) {
          newPos = "right";
        } else if (pref === "right" && rect.right > window.innerWidth) {
          newPos = "left";
        }

        if (newPos !== pref) {
          setActualPos(newPos);
        }
      });
    }, 400);
  };

  const hide = () => {
    clearTimeout(timeoutId);
    setIsVisible(false);
  };

  onCleanup(() => clearTimeout(timeoutId));

  const getPositionStyles = () => {
    switch (actualPos()) {
      case "bottom":
        return { top: "100%", left: "50%", transform: "translateX(-50%)", "margin-top": "6px" };
      case "left":
        return { top: "50%", right: "100%", transform: "translateY(-50%)", "margin-right": "6px" };
      case "right":
        return { top: "50%", left: "100%", transform: "translateY(-50%)", "margin-left": "6px" };
      case "top":
      default:
        return { bottom: "100%", left: "50%", transform: "translateX(-50%)", "margin-bottom": "6px" };
    }
  };

  return (
    <div
      ref={containerRef}
      style={{ position: "relative", display: "inline-block" }}
      onMouseEnter={show}
      onMouseLeave={hide}
      onFocusIn={show}
      onFocusOut={hide}
    >
      {props.children}
      <Show when={isVisible()}>
        <div
          ref={tooltipRef}
          style={{
            position: "absolute",
            ...getPositionStyles(),
            background: "var(--bg-popover, #333)",
            color: "var(--text-on-accent, #fff)",
            padding: "4px 8px",
            "border-radius": "4px",
            "font-size": "12px",
            "white-space": "nowrap",
            "z-index": "var(--z-tooltip, 1000)",
            "pointer-events": "none",
            display: "flex",
            "align-items": "center",
            gap: "8px",
            "box-shadow": "0 2px 4px rgba(0,0,0,0.2)",
          }}
        >
          <span>{props.content}</span>
          <Show when={props.shortcut}>
            <span style={{ opacity: 0.7, "font-size": "10px", padding: "2px 4px", background: "rgba(255,255,255,0.2)", "border-radius": "2px" }}>
              {props.shortcut}
            </span>
          </Show>
        </div>
      </Show>
    </div>
  );
}
