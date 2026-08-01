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

export const BubbleToolbar: Component<BubbleToolbarProps> = (props) => {
  return (
    <Show when={props.visible}>
      <div
        style={{
          position: "fixed",
          top: `${props.top}px`,
          left: `${props.left}px`,
          transform: "translate(-50%, -100%)",
          "margin-top": "-10px",
          display: "flex",
          "align-items": "center",
          gap: "4px",
          padding: "4px",
          background: "#ffffff",
          border: "1px solid #dadce0",
          "border-radius": "8px",
          "box-shadow": "0 4px 6px -1px rgb(0 0 0 / 0.1), 0 2px 4px -2px rgb(0 0 0 / 0.1)",
          "z-index": 1000,
        }}
      >
        <button
          onClick={(e) => { e.preventDefault(); props.onBold(); }}
          style={{
            background: "none",
            border: "none",
            padding: "4px",
            cursor: "pointer",
            display: "flex",
            "align-items": "center",
            "justify-content": "center",
            "border-radius": "4px",
          }}
          onMouseEnter={(e) => (e.currentTarget.style.backgroundColor = "#f3f4f6")}
          onMouseLeave={(e) => (e.currentTarget.style.backgroundColor = "transparent")}
          title="Bold"
        >
          <IconBold width={18} height={18} />
        </button>
        <button
          onClick={(e) => { e.preventDefault(); props.onItalic(); }}
          style={{
            background: "none",
            border: "none",
            padding: "4px",
            cursor: "pointer",
            display: "flex",
            "align-items": "center",
            "justify-content": "center",
            "border-radius": "4px",
          }}
          onMouseEnter={(e) => (e.currentTarget.style.backgroundColor = "#f3f4f6")}
          onMouseLeave={(e) => (e.currentTarget.style.backgroundColor = "transparent")}
          title="Italic"
        >
          <IconItalic width={18} height={18} />
        </button>
        <button
          onClick={(e) => { e.preventDefault(); props.onUnderline(); }}
          style={{
            background: "none",
            border: "none",
            padding: "4px",
            cursor: "pointer",
            display: "flex",
            "align-items": "center",
            "justify-content": "center",
            "border-radius": "4px",
          }}
          onMouseEnter={(e) => (e.currentTarget.style.backgroundColor = "#f3f4f6")}
          onMouseLeave={(e) => (e.currentTarget.style.backgroundColor = "transparent")}
          title="Underline"
        >
          <IconUnderline width={18} height={18} />
        </button>
        <button
          onClick={(e) => { e.preventDefault(); props.onStrikethrough(); }}
          style={{
            background: "none",
            border: "none",
            padding: "4px",
            cursor: "pointer",
            display: "flex",
            "align-items": "center",
            "justify-content": "center",
            "border-radius": "4px",
          }}
          onMouseEnter={(e) => (e.currentTarget.style.backgroundColor = "#f3f4f6")}
          onMouseLeave={(e) => (e.currentTarget.style.backgroundColor = "transparent")}
          title="Strikethrough"
        >
          <IconStrikethrough width={18} height={18} />
        </button>
        <button
          onClick={(e) => { e.preventDefault(); props.onLink(); }}
          style={{
            background: "none",
            border: "none",
            padding: "4px",
            cursor: "pointer",
            display: "flex",
            "align-items": "center",
            "justify-content": "center",
            "border-radius": "4px",
          }}
          onMouseEnter={(e) => (e.currentTarget.style.backgroundColor = "#f3f4f6")}
          onMouseLeave={(e) => (e.currentTarget.style.backgroundColor = "transparent")}
          title="Link"
        >
          <IconLink width={18} height={18} />
        </button>
      </div>
    </Show>
  );
};
