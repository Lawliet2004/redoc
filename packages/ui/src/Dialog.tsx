import { createEffect, onCleanup, ParentProps, Show } from "solid-js";
import { Button } from "./Button";

interface DialogProps {
  open: boolean;
  title: string;
  onClose: () => void;
}

export function Dialog(props: ParentProps<DialogProps>) {
  let dialogRef!: HTMLDivElement;
  let previouslyFocused: HTMLElement | null = null;
  const focusableSelector =
    'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

  const handleDialogKeyDown = (event: KeyboardEvent) => {
    if (event.key === "Escape") {
      event.preventDefault();
      props.onClose();
      return;
    }
    if (event.key !== "Tab") return;
    const focusable = Array.from(dialogRef.querySelectorAll<HTMLElement>(focusableSelector));
    if (!focusable.length) {
      event.preventDefault();
      dialogRef.focus();
      return;
    }
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  };

  createEffect(() => {
    if (props.open) {
      previouslyFocused = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      queueMicrotask(() => {
        const first = dialogRef?.querySelector<HTMLElement>(focusableSelector);
        (first || dialogRef)?.focus();
      });
    } else if (previouslyFocused) {
      previouslyFocused.focus();
      previouslyFocused = null;
    }
  });
  onCleanup(() => previouslyFocused?.focus());

  return (
    <Show when={props.open}>
      <div
        role="presentation"
        style={{
          position: "fixed",
          top: 0,
          left: 0,
          right: 0,
          bottom: 0,
          background: "rgba(0, 0, 0, 0.5)",
          display: "flex",
          "align-items": "center",
          "justify-content": "center",
          "z-index": 1000,
          "backdrop-filter": "blur(4px)",
        }}
        onClick={(e) => {
          if (e.target === e.currentTarget) props.onClose();
        }}
      >
        <div
          ref={dialogRef}
          role="dialog"
          aria-modal="true"
          aria-label={props.title}
          tabindex="-1"
          onKeyDown={handleDialogKeyDown}
          style={{
            background: "var(--bg-surface)",
            border: "1px solid var(--border-color)",
            "border-radius": "var(--radius-lg)",
            padding: "20px",
            width: "480px",
            "max-width": "90vw",
            "box-shadow": "var(--shadow-lg)",
            display: "flex",
            "flex-direction": "column",
            gap: "16px",
          }}
        >
          <div style={{ display: "flex", "justify-content": "space-between", "align-items": "center" }}>
            <h3 style={{ "font-size": "18px", "font-weight": "600", color: "var(--text-primary)" }}>
              {props.title}
            </h3>
            <Button variant="ghost" size="sm" aria-label="Close dialog" onClick={props.onClose}>✕</Button>
          </div>
          <div>{props.children}</div>
        </div>
      </div>
    </Show>
  );
}
