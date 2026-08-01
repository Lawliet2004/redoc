import { emitEditorCommand, repeatLastFormattingCommand } from "./EditorCommands";
import type { MenuDefinition } from "./MenuBar";

export interface CommandItem {
  id: string;
  title: string;
  shortcut?: string;
  mode?: "global" | "doc" | "sheet" | "slide";
  action: () => void;
  /** Top-level menu label then item label, e.g. ["File","Open…"] */
  menuPath?: string[];
  disabled?: () => boolean;
  separatorBefore?: boolean;
}

const MENU_ORDER = [
  "File",
  "Edit",
  "View",
  "Insert",
  "Format",
  "Styles",
  "Sheet",
  "Slide",
  "Table",
  "Tools",
  "Window",
  "Help",
];

class ShortcutRegistry {
  private commands: Map<string, CommandItem> = new Map();

  register(command: CommandItem) {
    this.commands.set(command.id, command);
  }

  unregister(id: string) {
    this.commands.delete(id);
  }

  getAll(): CommandItem[] {
    return Array.from(this.commands.values());
  }

  get(id: string): CommandItem | undefined {
    return this.commands.get(id);
  }

  run(id: string) {
    this.commands.get(id)?.action();
  }

  handleKeyDown(event: KeyboardEvent, currentMode: "doc" | "sheet" | "slide") {
    for (const cmd of this.commands.values()) {
      if (!cmd.shortcut) continue;
      if (cmd.mode && cmd.mode !== "global" && cmd.mode !== currentMode) continue;
      if (!matchShortcut(event, cmd.shortcut)) continue;
      event.preventDefault();
      cmd.action();
      return;
    }
  }

  /** Group registered commands with menuPath into MenuBar definitions. */
  buildMenus(mode: "home" | "doc" | "sheet" | "slide"): MenuDefinition[] {
    const groups = new Map<string, CommandItem[]>();
    for (const cmd of this.commands.values()) {
      if (!cmd.menuPath?.length) continue;
      const top = cmd.menuPath[0];
      // Mode-specific top menus only when active.
      if (top === "Sheet" && mode !== "sheet") continue;
      if (top === "Slide" && mode !== "slide") continue;
      if (top === "Table" && mode !== "doc" && mode !== "home") continue;
      if (cmd.mode && cmd.mode !== "global" && mode !== "home" && cmd.mode !== mode) {
        continue;
      }
      const list = groups.get(top) || [];
      list.push(cmd);
      groups.set(top, list);
    }

    const menus: MenuDefinition[] = [];
    const ordered = [
      ...MENU_ORDER.filter((label) => groups.has(label)),
      ...Array.from(groups.keys()).filter((label) => !MENU_ORDER.includes(label)),
    ];

    for (const label of ordered) {
      const cmds = groups.get(label) || [];
      const items: MenuDefinition["items"] = [];
      cmds.forEach((cmd, index) => {
        if (cmd.separatorBefore && index > 0) {
          items.push({ id: `${cmd.id}-sep`, label: "", separator: true });
        }
        items.push({
          id: cmd.id,
          label: cmd.menuPath?.[1] || cmd.title,
          shortcut: cmd.shortcut,
          disabled: cmd.disabled?.() ?? false,
          action: cmd.action,
        });
      });
      menus.push({
        id: label.toLowerCase().replace(/\s+/g, "-"),
        label,
        items,
      });
    }
    return menus;
  }
}

export function matchShortcut(e: KeyboardEvent, shortcut: string): boolean {
  const parts = shortcut.toLowerCase().split("+");
  const ctrl = parts.includes("ctrl") || parts.includes("cmd");
  const alt = parts.includes("alt");
  const shift = parts.includes("shift");
  const key = parts[parts.length - 1];

  return (
    (e.ctrlKey || e.metaKey) === ctrl &&
    e.altKey === alt &&
    e.shiftKey === shift &&
    e.key.toLowerCase() === key
  );
}

/** Register shell shortcuts that either call App handlers or emit editor commands. */
export function registerShellShortcuts(
  registry: ShortcutRegistry,
  handlers: {
    open: () => void;
    save: () => void;
    saveAs?: () => void;
  },
) {
  const ids = [
    "open-document",
    "save-document",
    "print",
    "find",
    "duplicate-slide",
    "bold",
    "italic",
    "underline",
  ];
  registry.register({
    id: "open-document",
    title: "Open",
    shortcut: "Ctrl+O",
    mode: "global",
    menuPath: ["File", "Open…"],
    action: handlers.open,
  });
  registry.register({
    id: "save-document",
    title: "Save Document",
    shortcut: "Ctrl+S",
    mode: "global",
    menuPath: ["File", "Save"],
    action: handlers.save,
  });
  if (handlers.saveAs) {
    registry.register({
      id: "save-as-document",
      title: "Save As…",
      shortcut: "Ctrl+Shift+S",
      mode: "global",
      menuPath: ["File", "Save As…"],
      action: handlers.saveAs,
    });
    ids.push("save-as-document");
  }
  registry.register({
    id: "print",
    title: "Print",
    shortcut: "Ctrl+P",
    mode: "global",
    menuPath: ["File", "Print"],
    action: () => emitEditorCommand("print"),
  });
  registry.register({
    id: "find",
    title: "Find",
    shortcut: "Ctrl+F",
    mode: "global",
    menuPath: ["Edit", "Find & Replace…"],
    action: () => emitEditorCommand("find"),
  });
  registry.register({
    id: "duplicate-slide",
    title: "Duplicate Slide",
    shortcut: "Ctrl+D",
    mode: "slide",
    menuPath: ["Slide", "Duplicate Slide"],
    action: () => emitEditorCommand("duplicate-slide"),
  });
  registry.register({
    id: "bold",
    title: "Bold",
    shortcut: "Ctrl+B",
    mode: "global",
    menuPath: ["Format", "Bold"],
    action: () => emitEditorCommand("bold"),
  });
  registry.register({
    id: "italic",
    title: "Italic",
    shortcut: "Ctrl+I",
    mode: "global",
    menuPath: ["Format", "Italic"],
    action: () => emitEditorCommand("italic"),
  });
  registry.register({
    id: "underline",
    title: "Underline",
    shortcut: "Ctrl+U",
    mode: "global",
    menuPath: ["Format", "Underline"],
    action: () => emitEditorCommand("underline"),
  });
  registry.register({
    id: "repeat-formatting",
    title: "Repeat Last Formatting",
    shortcut: "F4",
    mode: "global",
    menuPath: ["Format", "Repeat"],
    separatorBefore: true,
    action: () => repeatLastFormattingCommand(),
  });
  registry.register({
    id: "paste-values",
    title: "Paste Values",
    shortcut: "Ctrl+Shift+V",
    mode: "sheet",
    menuPath: ["Edit", "Paste Values"],
    action: () => emitEditorCommand("paste-values"),
  });
  registry.register({
    id: "sheet-goto-a1",
    title: "Go to A1",
    shortcut: "Ctrl+Home",
    mode: "sheet",
    action: () => emitEditorCommand("goto-a1"),
  });
  registry.register({
    id: "sheet-goto-last",
    title: "Go to Last Used Cell",
    shortcut: "Ctrl+End",
    mode: "sheet",
    action: () => emitEditorCommand("goto-last"),
  });
  registry.register({
    id: "sheet-jump-up",
    title: "Jump to Data Edge Up",
    shortcut: "Ctrl+ArrowUp",
    mode: "sheet",
    action: () => emitEditorCommand("jump-edge", { dRow: -1, dCol: 0 }),
  });
  registry.register({
    id: "sheet-jump-down",
    title: "Jump to Data Edge Down",
    shortcut: "Ctrl+ArrowDown",
    mode: "sheet",
    action: () => emitEditorCommand("jump-edge", { dRow: 1, dCol: 0 }),
  });
  registry.register({
    id: "sheet-jump-left",
    title: "Jump to Data Edge Left",
    shortcut: "Ctrl+ArrowLeft",
    mode: "sheet",
    action: () => emitEditorCommand("jump-edge", { dRow: 0, dCol: -1 }),
  });
  registry.register({
    id: "sheet-jump-right",
    title: "Jump to Data Edge Right",
    shortcut: "Ctrl+ArrowRight",
    mode: "sheet",
    action: () => emitEditorCommand("jump-edge", { dRow: 0, dCol: 1 }),
  });
  ids.push(
    "repeat-formatting",
    "paste-values",
    "sheet-goto-a1",
    "sheet-goto-last",
    "sheet-jump-up",
    "sheet-jump-down",
    "sheet-jump-left",
    "sheet-jump-right",
  );
  return ids;
}

export const shortcutRegistry = new ShortcutRegistry();

export function buildMenus(mode: "home" | "doc" | "sheet" | "slide"): MenuDefinition[] {
  return shortcutRegistry.buildMenus(mode);
}
