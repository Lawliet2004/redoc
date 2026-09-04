import { Show } from "solid-js";
import { Dialog } from "./Dialog";
import { Button } from "./Button";

interface ConfirmDialogProps {
  open: boolean;
  title: string;
  message: string;
  confirmLabel?: string;
  cancelLabel?: string;
  danger?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

export function ConfirmDialog(props: ConfirmDialogProps) {
  return (
    <Dialog open={props.open} title={props.title} onClose={props.onCancel}>
      <div style={{ display: "flex", "flex-direction": "column", gap: "16px" }}>
        <p style={{ "font-size": "13px", color: "var(--text-secondary)", margin: 0 }}>{props.message}</p>
        <div style={{ display: "flex", "justify-content": "flex-end", gap: "8px" }}>
          <Button variant="secondary" onClick={props.onCancel}>
            {props.cancelLabel || "Cancel"}
          </Button>
          <Button variant={props.danger ? "danger" : "primary"} onClick={props.onConfirm}>
            {props.confirmLabel || "Confirm"}
          </Button>
        </div>
      </div>
    </Dialog>
  );
}
