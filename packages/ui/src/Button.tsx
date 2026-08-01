import { JSX, ParentProps, splitProps } from "solid-js";

interface ButtonProps extends JSX.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "primary" | "secondary" | "ghost" | "danger" | "share";
  size?: "sm" | "md" | "lg";
}

export function Button(props: ParentProps<ButtonProps>) {
  const [local, others] = splitProps(props, ["variant", "size", "children", "style", "class"]);

  const variantStyle = () => {
    switch (local.variant) {
      case "secondary":
        return {
          background: "var(--bg-surface)",
          color: "var(--accent-color)",
          border: "1px solid var(--border-color)",
          "border-radius": "4px",
        };
      case "ghost":
        return {
          background: "transparent",
          color: "var(--text-primary)",
          border: "none",
          "border-radius": "4px",
        };
      case "danger":
        return {
          background: "var(--g-red, #d93025)",
          color: "#ffffff",
          border: "none",
          "border-radius": "4px",
        };
      case "share":
        return {
          background: "var(--accent-color)",
          color: "#ffffff",
          border: "none",
          "border-radius": "24px",
          "font-weight": "500",
        };
      default:
        return {
          background: "var(--accent-color)",
          color: "var(--accent-text)",
          border: "none",
          "border-radius": "4px",
        };
    }
  };

  const sizePadding = () => {
    if (local.variant === "share") return "8px 24px";
    switch (local.size) {
      case "sm":
        return "4px 10px";
      case "lg":
        return "10px 20px";
      default:
        return "6px 14px";
    }
  };

  return (
    <button
      {...others}
      class={local.class}
      style={{
        display: "inline-flex",
        "align-items": "center",
        "justify-content": "center",
        gap: "6px",
        padding: sizePadding(),
        "font-weight": local.variant === "share" ? "500" : "500",
        "font-size": local.size === "sm" ? "13px" : "14px",
        "letter-spacing": "0.15px",
        transition: "background 0.1s ease, box-shadow 0.1s ease",
        ...variantStyle(),
        ...(typeof local.style === "object" ? local.style : {}),
      }}
    >
      {local.children}
    </button>
  );
}
