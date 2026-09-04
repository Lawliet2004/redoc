import { Component, Show } from "solid-js";
import {
  IconBold,
  IconItalic,
  IconUnderline,
  IconStrikethrough,
  IconLink,
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
}

const buttonStyle = {
  background: "none",
  border: "none",
  padding: "4px",
  cursor: "pointer",
  display: "flex",
  "align-items": "center",
  "justify-content": "center",
  "border-radius": "4px",
  color: "inherit",
} as const;

export const BubbleToolbar: Component<BubbleToolbarProps> = (props) => {
  return (
    <Show when={props.visible}>
      <div
        class="g-bubble"
        style={{
          position: "fixed",
          top: `${props.top}px`,
          left: `${props.left}px`,
          transform: "translate(-50%, -100%)",
          "margin-top": "-10px",
        }}
      >
        <button onClick={(e) => { e.preventDefault(); props.onBold(); }} style={buttonStyle} title="Bold">
          <IconBold width={18} height={18} />
        </button>
        <button onClick={(e) => { e.preventDefault(); props.onItalic(); }} style={buttonStyle} title="Italic">
          <IconItalic width={18} height={18} />
        </button>
        <button onClick={(e) => { e.preventDefault(); props.onUnderline(); }} style={buttonStyle} title="Underline">
          <IconUnderline width={18} height={18} />
        </button>
        <button onClick={(e) => { e.preventDefault(); props.onStrikethrough(); }} style={buttonStyle} title="Strikethrough">
          <IconStrikethrough width={18} height={18} />
        </button>
        <button onClick={(e) => { e.preventDefault(); props.onLink(); }} style={buttonStyle} title="Link">
          <IconLink width={18} height={18} />
        </button>
      </div>
    </Show>
  );
};
