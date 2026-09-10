import { JSX, ParentProps, splitProps, Show, createSignal } from "solid-js";

export interface ButtonProps extends JSX.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "primary" | "secondary" | "ghost" | "danger" | "success" | "warning" | "link" | "share";
  size?: "xs" | "sm" | "md" | "lg";
  loading?: boolean;
  icon?: JSX.Element;
  iconPosition?: "left" | "right";
  fullWidth?: boolean;
}

export function Button(props: ParentProps<ButtonProps>) {
  const [local, others] = splitProps(props, [
    "variant",
    "size",
    "loading",
    "icon",
    "iconPosition",
    "fullWidth",
    "children",
    "style",
    "class",
    "disabled",
    "onClick",
  ]);

  const [isPressed, setIsPressed] = createSignal(false);

  const variantStyle = (): JSX.CSSProperties => {
    switch (local.variant) {
      case "secondary":
        return {
          background: "var(--bg-surface, #383838)",
          color: "var(--accent-color, #66b3ff)",
          border: "1px solid var(--border-color, #555)",
          "border-radius": "var(--radius-md, 6px)",
        };
      case "ghost":
        return {
          background: "transparent",
          color: "var(--text-primary, #f5f5f5)",
          border: "1px solid transparent",
          "border-radius": "var(--radius-md, 6px)",
        };
      case "danger":
        return {
          background: "var(--g-red, #d93025)",
          color: "#ffffff",
          border: "none",
          "border-radius": "var(--radius-md, 6px)",
        };
      case "success":
        return {
          background: "var(--g-green, #22c55e)",
          color: "#ffffff",
          border: "none",
          "border-radius": "var(--radius-md, 6px)",
        };
      case "warning":
        return {
          background: "var(--g-yellow, #f59e0b)",
          color: "#000000",
          border: "none",
          "border-radius": "var(--radius-md, 6px)",
        };
      case "link":
        return {
          background: "transparent",
          color: "var(--text-link, #8ecbff)",
          border: "none",
          "border-radius": "var(--radius-sm, 4px)",
          "text-decoration": "underline",
          "text-underline-offset": "2px",
        };
      case "share":
        return {
          background: "var(--accent-color, #66b3ff)",
          color: "#ffffff",
          border: "none",
          "border-radius": "24px",
          "font-weight": "500",
        };
      default: // primary
        return {
          background: "var(--accent-color, #66b3ff)",
          color: "var(--accent-text, #0b1420)",
          border: "none",
          "border-radius": "var(--radius-md, 6px)",
        };
    }
  };

  const sizeStyles = (): JSX.CSSProperties => {
    switch (local.size) {
      case "xs":
        return {
          padding: "2px 8px",
          "font-size": "var(--font-size-xs, 11px)",
          "min-height": "24px",
        };
      case "lg":
        return {
          padding: "10px 24px",
          "font-size": "var(--font-size-lg, 16px)",
          "min-height": "44px",
        };
      case "sm":
        return {
          padding: "4px 12px",
          "font-size": "var(--font-size-sm, 12px)",
          "min-height": "28px",
        };
      default: // md
        return {
          padding: "6px 16px",
          "font-size": "var(--font-size-base, 14px)",
          "min-height": "36px",
        };
    }
  };

  const isLoading = () => local.loading === true;
  const isDisabled = () => local.disabled === true || isLoading();

  return (
    <button
      {...others}
      class={`g-button ${local.class || ""}`.trim()}
      disabled={isDisabled()}
      aria-busy={isLoading()}
      aria-disabled={isDisabled()}
      onPointerDown={() => setIsPressed(true)}
      onPointerUp={() => setIsPressed(false)}
      onPointerLeave={() => setIsPressed(false)}
      onClick={(e) => {
        if (isDisabled()) {
          e.preventDefault();
          return;
        }
        if (local.onClick) {
          (local.onClick as (e: MouseEvent) => void)(e);
        }
      }}
      style={{
        display: "inline-flex",
        "align-items": "center",
        "justify-content": "center",
        gap: "var(--space-2, 8px)",
        "font-weight": "500",
        "letter-spacing": "0.01em",
        "line-height": "1.2",
        cursor: isDisabled() ? "not-allowed" : "pointer",
        opacity: isDisabled() && !isLoading() ? 0.5 : 1,
        width: local.fullWidth ? "100%" : "auto",
        transition: "background var(--duration-fast, 100ms) ease, box-shadow var(--duration-fast, 100ms) ease, opacity var(--duration-fast, 100ms) ease",
        "white-space": "nowrap",
        "user-select": "none",
        ...sizeStyles(),
        ...variantStyle(),
        ...(isPressed() && !isDisabled()
          ? { transform: "scale(0.98)" }
          : {}),
        ...(typeof local.style === "object" ? local.style : {}),
      }}
    >
      <Show when={isLoading()}>
        <span
          class="g-button-spinner"
          aria-hidden="true"
          style={{
            width: "1em",
            height: "1em",
            "border-width": "2px",
            "border-style": "solid",
            "border-color": "currentColor transparent transparent transparent",
            "border-radius": "50%",
            display: "inline-block",
            animation: "g-button-spin 0.6s linear infinite",
          }}
        />
      </Show>
      <Show when={local.icon && local.iconPosition !== "right"}>
        <span aria-hidden="true" style={{ display: "inline-flex", "flex-shrink": 0 }}>
          {local.icon}
        </span>
      </Show>
      <Show when={local.children}>
        <span>{local.children}</span>
      </Show>
      <Show when={local.icon && local.iconPosition === "right"}>
        <span aria-hidden="true" style={{ display: "inline-flex", "flex-shrink": 0 }}>
          {local.icon}
        </span>
      </Show>
    </button>
  );
}

// Icon-only button variant for compact toolbars
export interface IconButtonProps extends Omit<ButtonProps, "icon" | "iconPosition"> {
  icon: JSX.Element;
  tooltip?: string;
}

export function IconButton(props: ParentProps<IconButtonProps>) {
  const [local, others] = splitProps(props, ["icon", "tooltip", "size", "children", "style", "class"]);

  const iconSize = () => {
    switch (local.size) {
      case "xs":
        return "16px";
      case "lg":
        return "24px";
      case "sm":
        return "18px";
      default:
        return "20px";
    }
  };

  return (
    <Button
      {...others}
      class={`g-icon-button ${local.class || ""}`.trim()}
      title={local.tooltip}
      aria-label={local.tooltip}
      style={{
        padding: local.size === "xs" ? "2px" : local.size === "lg" ? "8px" : "4px",
        "min-width": iconSize(),
        "min-height": iconSize(),
        ...(typeof local.style === "object" ? local.style : {}),
      }}
    >
      <span style={{ display: "inline-flex" }}>{local.icon}</span>
    </Button>
  );
}
