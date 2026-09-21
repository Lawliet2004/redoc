import { emitEditorCommand, repeatLastFormattingCommand } from "./EditorCommands";

export interface MenuAction {
  id: string;
  label: string;
  shortcut?: string;
  disabled?: boolean;
  separator?: boolean;
  action?: () => void;
}

export interface MenuDefinition {
  id: string;
  label: string;
  items: MenuAction[];
}

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

export class ShortcutRegistry {
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
    const cmd = this.commands.get(id);
    if (!cmd || cmd.disabled?.()) return;
    cmd.action();
  }

  handleKeyDown(event: KeyboardEvent, currentMode: "doc" | "sheet" | "slide") {
    // Something upstream already consumed this key (e.g. a ProseMirror keymap
    // or an editor-level handler) — never double-fire a registered command.
    if (event.defaultPrevented || event.isComposing) return;
    // Plain form fields own every chord: typing Ctrl+B/V/Z in an input or
    // textarea must edit the field, not trigger editor commands.
    if (isFormFieldTarget(event.target)) return;
    // Editable surfaces (ProseMirror, slide text boxes) handle editing chords
    // natively; block just those so global chords like Ctrl+S/F/K still work.
    const editable = isEditableTarget(event.target);

    for (const cmd of this.commands.values()) {
      if (!cmd.shortcut) continue;
      if (cmd.mode && cmd.mode !== "global" && cmd.mode !== currentMode) continue;
      if (!matchShortcut(event, cmd.shortcut)) continue;
      // In an editable surface, chords the surface performs natively
      // (paste, undo, bold…) must not also fire via the registry.
      if (editable && isNativeEditingChord(cmd.shortcut)) return;
      if (cmd.disabled?.()) {
        // Disabled commands swallow their shortcut without running.
        event.preventDefault();
        return;
      }
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

/**
 * Characters that Shift+<key> produces on a standard ANSI layout, mapped back
 * to the unshifted key. Lets a shortcut declaring a base key ("=") still match
 * when the user presses Shift to produce the shifted glyph ("+") — the same
 * convention browsers use for zoom (Ctrl+= and Ctrl+Shift+= both zoom in).
 */
const UNSHIFT_KEY: Record<string, string> = {
  "~": "`",
  "!": "1",
  "@": "2",
  "#": "3",
  $: "4",
  "%": "5",
  "^": "6",
  "&": "7",
  "*": "8",
  "(": "9",
  ")": "0",
  _: "-",
  "+": "=",
  "{": "[",
  "}": "]",
  "|": "\\",
  ":": ";",
  '"': "'",
  "<": ",",
  ">": ".",
  "?": "/",
};

/** Input types that never receive typed text — shortcuts may pass through. */
const NON_TEXT_INPUT_TYPES = new Set([
  "button",
  "checkbox",
  "color",
  "file",
  "image",
  "radio",
  "range",
  "reset",
  "submit",
]);

/** Single-character editing chords native to inputs and contenteditable. */
const NATIVE_EDITING_KEYS = new Set(["a", "b", "c", "e", "i", "u", "v", "x", "y", "z"]);

/** True for text-entry form fields (input/textarea/select) — fields own all chords. */
function isFormFieldTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target instanceof HTMLTextAreaElement || target instanceof HTMLSelectElement) return true;
  if (target instanceof HTMLInputElement) {
    // Button-like inputs don't consume typing chords; let shortcuts through.
    return !NON_TEXT_INPUT_TYPES.has(target.type);
  }
  return false;
}

/** True for anything the user can type into, including contenteditable roots. */
function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return target.isContentEditable || isFormFieldTarget(target);
}

/**
 * True when `shortcut` is a plain Ctrl/Cmd(+Shift)+<char> chord that editable
 * surfaces already perform natively (clipboard, undo/redo, inline emphasis,
 * select-all). Alt-chords and non-editing keys are never treated as editing.
 */
function isNativeEditingChord(shortcut: string): boolean {
  const parts = shortcut.toLowerCase().split("+");
  if (!parts.includes("ctrl") && !parts.includes("cmd")) return false;
  if (parts.includes("alt")) return false;
  let key = parts[parts.length - 1];
  if (key === "") key = "=";
  return key.length === 1 && NATIVE_EDITING_KEYS.has(key);
}

export function matchShortcut(e: KeyboardEvent, shortcut: string): boolean {
  const parts = shortcut.toLowerCase().split("+");
  const ctrl = parts.includes("ctrl") || parts.includes("cmd");
  const alt = parts.includes("alt");
  const shift = parts.includes("shift");
  // "Ctrl+=" / "Ctrl+-" use the last part directly; "Ctrl++" splits to an
  // empty key part, so map it to the physical Equal key (unshifted "+").
  let key = parts[parts.length - 1];
  if (key === "") key = "=";
  const code = e.code.replace(/Key|Digit|Numpad/, "").toLowerCase();
  const eventKey = e.key.toLowerCase();

  if ((e.ctrlKey || e.metaKey) !== ctrl || e.altKey !== alt) return false;

  if (eventKey === key) {
    // The produced character equals the declared key. For printable keys the
    // produced char already encodes Shift (Shift+= yields "+"), so a declared
    // shifted symbol is satisfied by the glyph itself; otherwise demand exact
    // Shift equality (keeps Ctrl+A and Ctrl+Shift+A distinct).
    if (e.shiftKey === shift) return true;
    return key.length === 1 && Object.prototype.hasOwnProperty.call(UNSHIFT_KEY, key);
  }

  // The event produced a shifted symbol whose unshifted key is the declared
  // one — e.g. declared "Ctrl+=" (or "Ctrl+Shift+="/"Ctrl++") and the press
  // Shift+= produced "+". Shift is already encoded in the glyph, so this also
  // covers dedicated "+" keys (e.g. Numpad+) that produce it without Shift.
  if (UNSHIFT_KEY[eventKey] === key) return true;

  // Fall back to physical-position matching for named keys / other layouts.
  return e.shiftKey === shift && code === key;
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
