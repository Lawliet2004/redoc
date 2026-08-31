export const EDITOR_COMMAND = "redoc:editor-command";

export type EditorCommandId =
  | "find"
  | "find-replace"
  | "insert-image"
  | "insert-table"
  | "insert-link"
  | "insert-textbox"
  | "insert-chart"
  | "insert-page-break"
  | "add-comment"
  | "bold"
  | "italic"
  | "underline"
  | "clear-formatting"
  | "style-default"
  | "style-h1"
  | "style-h2"
  | "new-slide"
  | "duplicate-slide"
  | "delete-slide"
  | "slide-layout"
  | "present"
  | "sort-asc"
  | "filter"
  | "print"
  | "insert-sheet"
  | "paste-values";

export interface EditorCommandDetail {
  id: EditorCommandId | string;
  payload?: unknown;
}

/** Formatting commands eligible for F4 repeat. */
const FORMATTING_COMMANDS = new Set<string>([
  "bold",
  "italic",
  "underline",
  "clear-formatting",
  "style-default",
  "style-h1",
  "style-h2",
]);

let lastFormattingCommandId: EditorCommandId | string | null = null;

export function getLastFormattingCommand(): EditorCommandId | string | null {
  return lastFormattingCommandId;
}

export function emitEditorCommand(id: EditorCommandId | string, payload?: unknown) {
  if (FORMATTING_COMMANDS.has(id)) {
    lastFormattingCommandId = id;
  }
  window.dispatchEvent(
    new CustomEvent<EditorCommandDetail>(EDITOR_COMMAND, {
      detail: { id, payload },
    }),
  );
}

export function repeatLastFormattingCommand() {
  if (!lastFormattingCommandId) return;
  window.dispatchEvent(
    new CustomEvent<EditorCommandDetail>(EDITOR_COMMAND, {
      detail: { id: lastFormattingCommandId },
    }),
  );
}
