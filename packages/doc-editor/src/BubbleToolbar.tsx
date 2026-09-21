import { Component, Show, JSX } from "solid-js";
import {
  IconBold,
  IconItalic,
  IconUnderline,
  IconStrikethrough,
  IconLink,
  IconHighlight,
} from "@redoc/icons";

interface BubbleToolbarProps {
  visible: boolean;
  top: number;
  left: number;
  onBold: () => void;
  onItalic: () => void;
  onUnderline: () => void;
  onStrikethrough: () => void;
  onLink: () => void;
  /** Applies the current highlight color to the selection (toggles). */
  onHighlight?: () => void;
  /** Adds a comment anchored to the current selection. */
  onComment?: () => void;
}

/** Comment glyph — lucide "message-square" in the shared stroke style. */
function IconComment() {
  return (
    <svg
      width="16"
      height="16"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="2"
      stroke-linecap="round"
      stroke-linejoin="round"
      aria-hidden="true"
    >
      <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
    </svg>
  );
}

function BubbleButton(props: { title: string; onClick: () => void; children: JSX.Element }) {
  return (
    <button
      type="button"
      class="doc-bubble-btn"
      title={props.title}
      aria-label={props.title}
      // Keep the editor selection + focus; stealing it collapses the range
      // before the formatting command runs.
      onMouseDown={(e) => e.preventDefault()}
      onClick={(e) => {
        e.preventDefault();
        props.onClick();
      }}
    >
      {props.children}
    </button>
  );
}

export const BubbleToolbar: Component<BubbleToolbarProps> = (props) => {
  return (
    <Show when={props.visible}>
      <div
        class="g-bubble doc-bubble"
        role="toolbar"
        aria-label="Text formatting"
        style={{
          position: "fixed",
          top: `${props.top}px`,
          left: `${props.left}px`,
          transform: "translate(-50%, -100%)",
          "margin-top": "-10px",
        }}
      >
        <BubbleButton title="Bold" onClick={props.onBold}>
          <IconBold width={16} height={16} />
        </BubbleButton>
        <BubbleButton title="Italic" onClick={props.onItalic}>
          <IconItalic width={16} height={16} />
        </BubbleButton>
        <BubbleButton title="Underline" onClick={props.onUnderline}>
          <IconUnderline width={16} height={16} />
        </BubbleButton>
        <BubbleButton title="Strikethrough" onClick={props.onStrikethrough}>
          <IconStrikethrough width={16} height={16} />
        </BubbleButton>
        <span class="doc-bubble-sep" aria-hidden="true" />
        <BubbleButton title="Highlight" onClick={() => props.onHighlight?.()}>
          <IconHighlight width={16} height={16} />
        </BubbleButton>
        <BubbleButton title="Insert link" onClick={props.onLink}>
          <IconLink width={16} height={16} />
        </BubbleButton>
        <BubbleButton title="Add comment" onClick={() => props.onComment?.()}>
          <IconComment />
        </BubbleButton>
      </div>
    </Show>
  );
};
