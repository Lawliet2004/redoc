import { createSignal, For, Show } from "solid-js";

export interface ToastMessage {
  id: string;
  type: "info" | "success" | "error";
  text: string;
}

const [toasts, setToasts] = createSignal<ToastMessage[]>([]);

export function showToast(text: string, type: "info" | "success" | "error" = "info") {
  const id = Math.random().toString(36).slice(2);
  setToasts((prev) => [...prev, { id, type, text }]);
  setTimeout(() => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, 3500);
}

export function ToastContainer() {
  return (
    <div
      style={{
        position: "fixed",
        bottom: "20px",
        right: "20px",
        display: "flex",
        "flex-direction": "column",
        gap: "8px",
        "z-index": 2000,
      }}
    >
      <For each={toasts()}>
        {(toast) => (
          <div
            style={{
              padding: "10px 16px",
              "border-radius": "var(--radius-md)",
              background: toast.type === "error" ? "#ef4444" : toast.type === "success" ? "#10b981" : "var(--bg-tertiary)",
              color: toast.type === "info" ? "var(--text-primary)" : "#ffffff",
              "box-shadow": "var(--shadow-md)",
              "font-size": "13px",
              "font-weight": "500",
              animation: "fadeIn 0.2s ease-in-out",
            }}
          >
            {toast.text}
          </div>
        )}
      </For>
    </div>
  );
}
