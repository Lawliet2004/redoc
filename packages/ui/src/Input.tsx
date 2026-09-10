import { JSX, ParentProps, splitProps, Show, createUniqueId, Accessor, For } from "solid-js";

export interface InputProps extends Omit<JSX.InputHTMLAttributes<HTMLInputElement>, "size"> {
  label?: string;
  helperText?: string;
  error?: string | Accessor<string>;
  size?: "sm" | "md" | "lg";
  prefixIcon?: JSX.Element;
  suffixIcon?: JSX.Element;
  fullWidth?: boolean;
}

export function Input(props: ParentProps<InputProps>) {
  const [local, others] = splitProps(props, [
    "label",
    "helperText",
    "error",
    "size",
    "prefixIcon",
    "suffixIcon",
    "fullWidth",
    "children",
    "style",
    "class",
    "id",
  ]);

  const inputId = local.id || createUniqueId();
  const errorId = `${inputId}-error`;
  const helperId = `${inputId}-helper`;

  const errorMessage = () => {
    if (!local.error) return undefined;
    return typeof local.error === "function" ? local.error() : local.error;
  };

  const hasError = () => !!errorMessage();

  const sizeStyles = (): JSX.CSSProperties => {
    switch (local.size) {
      case "sm":
        return {
          padding: "4px 8px",
          "font-size": "var(--font-size-sm, 12px)",
          "min-height": "28px",
        };
      case "lg":
        return {
          padding: "10px 14px",
          "font-size": "var(--font-size-lg, 16px)",
          "min-height": "44px",
        };
      default: // md
        return {
          padding: "6px 10px",
          "font-size": "var(--font-size-base, 14px)",
          "min-height": "36px",
        };
    }
  };

  const inputStyles = (): JSX.CSSProperties => ({
    width: "100%",
    "box-sizing": "border-box",
    background: "var(--bg-input, #1e1e1e)",
    color: "var(--text-primary, #f5f5f5)",
    border: `1px solid ${hasError() ? "var(--g-red, #d93025)" : "var(--border-color, #555)"}`,
    "border-radius": "var(--radius-md, 6px)",
    outline: "none",
    transition: "border-color var(--duration-fast, 100ms) ease, box-shadow var(--duration-fast, 100ms) ease",
    "font-family": "var(--font-sans, system-ui, sans-serif)",
    ...sizeStyles(),
  });

  return (
    <div
      class={`g-input-wrapper ${local.class || ""}`.trim()}
      style={{
        display: "flex",
        "flex-direction": "column",
        gap: "var(--space-1, 4px)",
        width: local.fullWidth ? "100%" : "auto",
        ...(typeof local.style === "object" ? local.style : {}),
      }}
    >
      <Show when={local.label}>
        <label
          for={inputId}
          style={{
            "font-size": "var(--font-size-sm, 12px)",
            "font-weight": "500",
            color: "var(--text-secondary, #d4d4d4)",
            "user-select": "none",
          }}
        >
          {local.label}
        </label>
      </Show>
      <div
        style={{
          display: "flex",
          "align-items": "center",
          gap: "var(--space-2, 8px)",
          position: "relative",
        }}
      >
        <Show when={local.prefixIcon}>
          <span
            aria-hidden="true"
            style={{
              display: "inline-flex",
              "flex-shrink": 0,
              color: "var(--text-muted, #94a3b8)",
              "font-size": "var(--font-size-sm, 12px)",
            }}
          >
            {local.prefixIcon}
          </span>
        </Show>
        <input
          {...others}
          id={inputId}
          class="g-input"
          aria-invalid={hasError()}
          aria-describedby={
            [hasError() ? errorId : null, local.helperText ? helperId : null]
              .filter(Boolean)
              .join(" ") || undefined
          }
          style={inputStyles()}
          onFocus={(e) => {
            e.currentTarget.style.borderColor = hasError()
              ? "var(--g-red, #d93025)"
              : "var(--border-focus, #66b3ff)";
            e.currentTarget.style.boxShadow = hasError()
              ? "0 0 0 3px rgba(217, 48, 37, 0.2)"
              : "0 0 0 3px rgba(102, 179, 255, 0.2)";
          }}
          onBlur={(e) => {
            e.currentTarget.style.borderColor = hasError()
              ? "var(--g-red, #d93025)"
              : "var(--border-color, #555)";
            e.currentTarget.style.boxShadow = "none";
          }}
        />
        <Show when={local.suffixIcon}>
          <span
            aria-hidden="true"
            style={{
              display: "inline-flex",
              "flex-shrink": 0,
              color: "var(--text-muted, #94a3b8)",
              "font-size": "var(--font-size-sm, 12px)",
            }}
          >
            {local.suffixIcon}
          </span>
        </Show>
      </div>
      <Show when={hasError()}>
        <p
          id={errorId}
          role="alert"
          aria-live="polite"
          style={{
            margin: 0,
            "font-size": "var(--font-size-xs, 11px)",
            color: "var(--g-red, #d93025)",
          }}
        >
          {errorMessage()}
        </p>
      </Show>
      <Show when={local.helperText && !hasError()}>
        <p
          id={helperId}
          style={{
            margin: 0,
            "font-size": "var(--font-size-xs, 11px)",
            color: "var(--text-muted, #94a3b8)",
          }}
        >
          {local.helperText}
        </p>
      </Show>
    </div>
  );
}

export interface TextareaProps extends Omit<JSX.TextareaHTMLAttributes<HTMLTextAreaElement>, "size"> {
  label?: string;
  helperText?: string;
  error?: string | Accessor<string>;
  size?: "sm" | "md" | "lg";
  fullWidth?: boolean;
  resize?: "none" | "both" | "horizontal" | "vertical";
}

export function Textarea(props: ParentProps<TextareaProps>) {
  const [local, others] = splitProps(props, [
    "label",
    "helperText",
    "error",
    "size",
    "fullWidth",
    "resize",
    "children",
    "style",
    "class",
    "id",
  ]);

  const inputId = local.id || createUniqueId();
  const errorId = `${inputId}-error`;
  const helperId = `${inputId}-helper`;

  const errorMessage = () => {
    if (!local.error) return undefined;
    return typeof local.error === "function" ? local.error() : local.error;
  };

  const hasError = () => !!errorMessage();

  const sizeStyles = (): JSX.CSSProperties => {
    switch (local.size) {
      case "sm":
        return {
          padding: "4px 8px",
          "font-size": "var(--font-size-sm, 12px)",
          "min-height": "60px",
        };
      case "lg":
        return {
          padding: "10px 14px",
          "font-size": "var(--font-size-lg, 16px)",
          "min-height": "120px",
        };
      default: // md
        return {
          padding: "6px 10px",
          "font-size": "var(--font-size-base, 14px)",
          "min-height": "80px",
        };
    }
  };

  return (
    <div
      class={`g-input-wrapper ${local.class || ""}`.trim()}
      style={{
        display: "flex",
        "flex-direction": "column",
        gap: "var(--space-1, 4px)",
        width: local.fullWidth ? "100%" : "auto",
        ...(typeof local.style === "object" ? local.style : {}),
      }}
    >
      <Show when={local.label}>
        <label
          for={inputId}
          style={{
            "font-size": "var(--font-size-sm, 12px)",
            "font-weight": "500",
            color: "var(--text-secondary, #d4d4d4)",
            "user-select": "none",
          }}
        >
          {local.label}
        </label>
      </Show>
      <textarea
        {...others}
        id={inputId}
        class="g-input g-textarea"
        aria-invalid={hasError()}
        aria-describedby={
          [hasError() ? errorId : null, local.helperText ? helperId : null]
            .filter(Boolean)
            .join(" ") || undefined
        }
        style={{
          width: "100%",
          "box-sizing": "border-box",
          background: "var(--bg-input, #1e1e1e)",
          color: "var(--text-primary, #f5f5f5)",
          border: `1px solid ${hasError() ? "var(--g-red, #d93025)" : "var(--border-color, #555)"}`,
          "border-radius": "var(--radius-md, 6px)",
          outline: "none",
          resize: local.resize || "vertical",
          "font-family": "var(--font-sans, system-ui, sans-serif)",
          "line-height": "1.5",
          transition: "border-color var(--duration-fast, 100ms) ease, box-shadow var(--duration-fast, 100ms) ease",
          ...sizeStyles(),
        }}
        onFocus={(e) => {
          e.currentTarget.style.borderColor = hasError()
            ? "var(--g-red, #d93025)"
            : "var(--border-focus, #66b3ff)";
          e.currentTarget.style.boxShadow = hasError()
            ? "0 0 0 3px rgba(217, 48, 37, 0.2)"
            : "0 0 0 3px rgba(102, 179, 255, 0.2)";
        }}
        onBlur={(e) => {
          e.currentTarget.style.borderColor = hasError()
            ? "var(--g-red, #d93025)"
            : "var(--border-color, #555)";
          e.currentTarget.style.boxShadow = "none";
        }}
      />
      <Show when={hasError()}>
        <p
          id={errorId}
          role="alert"
          aria-live="polite"
          style={{
            margin: 0,
            "font-size": "var(--font-size-xs, 11px)",
            color: "var(--g-red, #d93025)",
          }}
        >
          {errorMessage()}
        </p>
      </Show>
      <Show when={local.helperText && !hasError()}>
        <p
          id={helperId}
          style={{
            margin: 0,
            "font-size": "var(--font-size-xs, 11px)",
            color: "var(--text-muted, #94a3b8)",
          }}
        >
          {local.helperText}
        </p>
      </Show>
    </div>
  );
}

export interface SelectProps extends Omit<JSX.SelectHTMLAttributes<HTMLSelectElement>, "size"> {
  label?: string;
  helperText?: string;
  error?: string | Accessor<string>;
  size?: "sm" | "md" | "lg";
  fullWidth?: boolean;
  options: Array<{ value: string; label: string; disabled?: boolean }>;
}

export function Select(props: ParentProps<SelectProps>) {
  const [local, others] = splitProps(props, [
    "label",
    "helperText",
    "error",
    "size",
    "fullWidth",
    "options",
    "children",
    "style",
    "class",
    "id",
  ]);

  const inputId = local.id || createUniqueId();
  const errorId = `${inputId}-error`;
  const helperId = `${inputId}-helper`;

  const errorMessage = () => {
    if (!local.error) return undefined;
    return typeof local.error === "function" ? local.error() : local.error;
  };

  const hasError = () => !!errorMessage();

  const sizeStyles = (): JSX.CSSProperties => {
    switch (local.size) {
      case "sm":
        return {
          padding: "4px 8px",
          "font-size": "var(--font-size-sm, 12px)",
          "min-height": "28px",
        };
      case "lg":
        return {
          padding: "10px 14px",
          "font-size": "var(--font-size-lg, 16px)",
          "min-height": "44px",
        };
      default: // md
        return {
          padding: "6px 10px",
          "font-size": "var(--font-size-base, 14px)",
          "min-height": "36px",
        };
    }
  };

  return (
    <div
      class={`g-input-wrapper ${local.class || ""}`.trim()}
      style={{
        display: "flex",
        "flex-direction": "column",
        gap: "var(--space-1, 4px)",
        width: local.fullWidth ? "100%" : "auto",
        ...(typeof local.style === "object" ? local.style : {}),
      }}
    >
      <Show when={local.label}>
        <label
          for={inputId}
          style={{
            "font-size": "var(--font-size-sm, 12px)",
            "font-weight": "500",
            color: "var(--text-secondary, #d4d4d4)",
            "user-select": "none",
          }}
        >
          {local.label}
        </label>
      </Show>
      <select
        {...others}
        id={inputId}
        class="g-input g-select"
        aria-invalid={hasError()}
        aria-describedby={
          [hasError() ? errorId : null, local.helperText ? helperId : null]
            .filter(Boolean)
            .join(" ") || undefined
        }
        style={{
          width: "100%",
          "box-sizing": "border-box",
          background: "var(--bg-input, #1e1e1e)",
          color: "var(--text-primary, #f5f5f5)",
          border: `1px solid ${hasError() ? "var(--g-red, #d93025)" : "var(--border-color, #555)"}`,
          "border-radius": "var(--radius-md, 6px)",
          outline: "none",
          cursor: "pointer",
          "font-family": "var(--font-sans, system-ui, sans-serif)",
          appearance: "none",
          "background-image": `url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 12 12'%3E%3Cpath fill='%2394a3b8' d='M6 8L1 3h10z'/%3E%3C/svg%3E")`,
          "background-repeat": "no-repeat",
          "background-position": "right 10px center",
          "padding-right": "28px",
          ...sizeStyles(),
        }}
      >
        <For each={local.options}>
          {(option) => (
            <option value={option.value} disabled={option.disabled}>
              {option.label}
            </option>
          )}
        </For>
      </select>
      <Show when={hasError()}>
        <p
          id={errorId}
          role="alert"
          aria-live="polite"
          style={{
            margin: 0,
            "font-size": "var(--font-size-xs, 11px)",
            color: "var(--g-red, #d93025)",
          }}
        >
          {errorMessage()}
        </p>
      </Show>
      <Show when={local.helperText && !hasError()}>
        <p
          id={helperId}
          style={{
            margin: 0,
            "font-size": "var(--font-size-xs, 11px)",
            color: "var(--text-muted, #94a3b8)",
          }}
        >
          {local.helperText}
        </p>
      </Show>
    </div>
  );
}
