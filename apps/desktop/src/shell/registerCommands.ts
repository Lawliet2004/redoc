import { emitEditorCommand, type CommandItem } from "@redoc/editor-common";
import { commands, type AppSettings } from "@redoc/api-client";
import { showToast } from "@redoc/ui";

export type EditorMode = "doc" | "sheet" | "slide";

export interface AppCommandContext {
  activeMode: () => "home" | EditorMode;
  zoomLevel: () => number;
  setZoomLevel: (v: number) => void;
  settings: () => AppSettings;
  statusInfo: () => string;
  docContent: () => any;
  handleNewDoc: (mode: EditorMode, templateId?: string) => void;
  handleSaveAs: () => void;
  handleSave: () => void;
  handleOpenFile: () => void;
  setExportOpen: (v: boolean) => void;
  goHome: () => void | Promise<void>;
  setPaletteOpen: (v: boolean) => void;
  setSettingsOpen: (v: boolean) => void;
  setHelpOpen: (v: boolean) => void;
  setAboutOpen: (v: boolean) => void;
  handleSwitchMode: (mode: EditorMode) => void;
}

export function buildAppCommands(ctx: AppCommandContext): CommandItem[] {
  return [
      {
        id: "new-document",
        title: "New Document",
        mode: "global" as const,
        menuPath: ["File", "New Document"],
        action: () => void ctx.handleNewDoc("doc"),
      },
      {
        id: "new-spreadsheet",
        title: "New Spreadsheet",
        mode: "global" as const,
        menuPath: ["File", "New Spreadsheet"],
        action: () => void ctx.handleNewDoc("sheet"),
      },
      {
        id: "new-presentation",
        title: "New Presentation",
        mode: "global" as const,
        menuPath: ["File", "New Presentation"],
        action: () => void ctx.handleNewDoc("slide"),
      },
      {
        id: "save-as-document",
        title: "Save As…",
        shortcut: "Ctrl+Shift+S",
        mode: "global" as const,
        menuPath: ["File", "Save As…"],
        action: () => void ctx.handleSaveAs(),
      },
      {
        id: "export-as",
        title: "Export as…",
        mode: "global" as const,
        menuPath: ["File", "Export as…"],
        separatorBefore: true,
        action: () => ctx.setExportOpen(true),
      },
      {
        id: "go-home",
        title: "Go to Home",
        shortcut: "Ctrl+Alt+H",
        mode: "global" as const,
        menuPath: ["File", "Home"],
        separatorBefore: true,
        action: () => void ctx.goHome(),
      },
      {
        id: "undo",
        title: "Undo",
        shortcut: "Ctrl+Z",
        mode: "global" as const,
        menuPath: ["Edit", "Undo"],
        action: () => emitEditorCommand("undo"),
      },
      {
        id: "redo",
        title: "Redo",
        shortcut: "Ctrl+Y",
        mode: "global" as const,
        menuPath: ["Edit", "Redo"],
        action: () => emitEditorCommand("redo"),
      },
      {
        id: "cut",
        title: "Cut",
        shortcut: "Ctrl+X",
        mode: "global" as const,
        menuPath: ["Edit", "Cut"],
        separatorBefore: true,
        action: () => emitEditorCommand("cut"),
      },
      {
        id: "copy",
        title: "Copy",
        shortcut: "Ctrl+C",
        mode: "global" as const,
        menuPath: ["Edit", "Copy"],
        action: () => emitEditorCommand("copy"),
      },
      {
        id: "paste",
        title: "Paste",
        shortcut: "Ctrl+V",
        mode: "global" as const,
        menuPath: ["Edit", "Paste"],
        action: () => emitEditorCommand("paste"),
      },
      {
        id: "find",
        title: "Find…",
        shortcut: "Ctrl+F",
        mode: "global" as const,
        menuPath: ["Edit", "Find…"],
        separatorBefore: true,
        action: () => emitEditorCommand("find"),
      },
      {
        id: "replace",
        title: "Replace…",
        shortcut: "Ctrl+H",
        mode: "global" as const,
        menuPath: ["Edit", "Replace…"],
        action: () => emitEditorCommand("find-replace"),
      },
      {
        id: "command-palette",
        title: "Command Palette",
        shortcut: "Ctrl+K",
        mode: "global" as const,
        menuPath: ["Edit", "Command Palette"],
        separatorBefore: true,
        action: () => ctx.setPaletteOpen(true),
      },
      {
        id: "zoom-in",
        title: "Zoom In",
        shortcut: "Ctrl+=",
        mode: "global" as const,
        menuPath: ["View", "Zoom In"],
        action: () => {
          const next = Math.min(200, ctx.zoomLevel() + 10);
          ctx.setZoomLevel(next);
          void commands.updateSettings({ ...ctx.settings(), zoomLevel: next }).catch(() => undefined);
        },
      },
      {
        id: "zoom-out",
        title: "Zoom Out",
        shortcut: "Ctrl+-",
        mode: "global" as const,
        menuPath: ["View", "Zoom Out"],
        action: () => {
          const next = Math.max(50, ctx.zoomLevel() - 10);
          ctx.setZoomLevel(next);
          void commands.updateSettings({ ...ctx.settings(), zoomLevel: next }).catch(() => undefined);
        },
      },
      {
        id: "zoom-100",
        title: "Zoom 100%",
        shortcut: "Ctrl+0",
        mode: "global" as const,
        menuPath: ["View", "Zoom 100%"],
        action: () => {
          ctx.setZoomLevel(100);
          void commands.updateSettings({ ...ctx.settings(), zoomLevel: 100 }).catch(() => undefined);
        },
      },
      {
        id: "insert-image",
        title: "Image…",
        mode: "global" as const,
        menuPath: ["Insert", "Image…"],
        disabled: () => ctx.activeMode() === "home",
        action: () => emitEditorCommand("insert-image"),
      },
      {
        id: "insert-table",
        title: "Table",
        mode: "doc" as const,
        menuPath: ["Insert", "Table"],
        disabled: () => ctx.activeMode() !== "doc",
        action: () => emitEditorCommand("insert-table"),
      },
      {
        id: "insert-link",
        title: "Hyperlink…",
        mode: "doc" as const,
        menuPath: ["Insert", "Hyperlink…"],
        disabled: () => ctx.activeMode() !== "doc",
        action: () => emitEditorCommand("insert-link"),
      },
      {
        id: "insert-bookmark",
        title: "Bookmark…",
        mode: "doc" as const,
        menuPath: ["Insert", "Bookmark…"],
        disabled: () => ctx.activeMode() !== "doc",
        action: () => emitEditorCommand("insert-bookmark"),
      },
      {
        id: "insert-chart",
        title: "Chart…",
        mode: "sheet" as const,
        menuPath: ["Insert", "Chart…"],
        disabled: () => ctx.activeMode() !== "sheet",
        action: () => emitEditorCommand("insert-chart"),
      },
      {
        id: "insert-page-break",
        title: "Page Break",
        mode: "doc" as const,
        menuPath: ["Insert", "Page Break"],
        disabled: () => ctx.activeMode() !== "doc",
        action: () => emitEditorCommand("insert-page-break"),
      },
      {
        id: "insert-page-field",
        title: "Page Number Field",
        mode: "doc" as const,
        menuPath: ["Insert", "Field", "Page Number"],
        disabled: () => ctx.activeMode() !== "doc",
        action: () => emitEditorCommand("insert-page-field"),
      },
      {
        id: "insert-num-pages-field",
        title: "Total Pages Field",
        mode: "doc" as const,
        menuPath: ["Insert", "Field", "Total Pages"],
        disabled: () => ctx.activeMode() !== "doc",
        action: () => emitEditorCommand("insert-num-pages-field"),
      },
      {
        id: "insert-toc",
        title: "Table of Contents",
        mode: "doc" as const,
        menuPath: ["Insert", "Table of Contents"],
        disabled: () => ctx.activeMode() !== "doc",
        action: () => emitEditorCommand("insert-toc"),
      },
      {
        id: "add-comment",
        title: "New Comment",
        mode: "doc" as const,
        menuPath: ["Review", "New Comment"],
        disabled: () => ctx.activeMode() !== "doc",
        action: () => emitEditorCommand("add-comment"),
      },
      {
        id: "mark-insertion",
        title: "Mark Selection as Insertion",
        mode: "doc" as const,
        menuPath: ["Review", "Track Changes", "Mark Insertion"],
        disabled: () => ctx.activeMode() !== "doc",
        action: () => emitEditorCommand("mark-insertion"),
      },
      {
        id: "mark-deletion",
        title: "Mark Selection as Deletion",
        mode: "doc" as const,
        menuPath: ["Review", "Track Changes", "Mark Deletion"],
        disabled: () => ctx.activeMode() !== "doc",
        action: () => emitEditorCommand("mark-deletion"),
      },
      {
        id: "accept-all-changes",
        title: "Accept All Changes",
        mode: "doc" as const,
        menuPath: ["Review", "Track Changes", "Accept All Changes"],
        disabled: () => ctx.activeMode() !== "doc",
        action: () => emitEditorCommand("accept-all-changes"),
      },
      {
        id: "reject-all-changes",
        title: "Reject All Changes",
        mode: "doc" as const,
        menuPath: ["Review", "Track Changes", "Reject All Changes"],
        disabled: () => ctx.activeMode() !== "doc",
        action: () => emitEditorCommand("reject-all-changes"),
      },
      {
        id: "clear-formatting",
        title: "Clear Direct Formatting",
        mode: "global" as const,
        menuPath: ["Format", "Clear Direct Formatting"],
        separatorBefore: true,
        action: () => emitEditorCommand("clear-formatting"),
      },
      {
        id: "style-default",
        title: "Default Paragraph Style",
        mode: "doc" as const,
        menuPath: ["Styles", "Default Paragraph Style"],
        action: () => emitEditorCommand("style-default"),
      },
      {
        id: "style-h1",
        title: "Heading 1",
        mode: "doc" as const,
        menuPath: ["Styles", "Heading 1"],
        action: () => emitEditorCommand("style-h1"),
      },
      {
        id: "style-h2",
        title: "Heading 2",
        mode: "doc" as const,
        menuPath: ["Styles", "Heading 2"],
        action: () => emitEditorCommand("style-h2"),
      },
      {
        id: "style-h3",
        title: "Heading 3",
        mode: "doc" as const,
        menuPath: ["Styles", "Heading 3"],
        action: () => emitEditorCommand("style-h3"),
      },
      {
        id: "style-h4",
        title: "Heading 4",
        mode: "doc" as const,
        menuPath: ["Styles", "Heading 4"],
        action: () => emitEditorCommand("style-h4"),
      },
      {
        id: "style-h5",
        title: "Heading 5",
        mode: "doc" as const,
        menuPath: ["Styles", "Heading 5"],
        action: () => emitEditorCommand("style-h5"),
      },
      {
        id: "style-h6",
        title: "Heading 6",
        mode: "doc" as const,
        menuPath: ["Styles", "Heading 6"],
        action: () => emitEditorCommand("style-h6"),
      },
      {
        id: "print-preview",
        title: "Print Preview",
        mode: "doc" as const,
        menuPath: ["File", "Print Preview"],
        disabled: () => ctx.activeMode() !== "doc",
        action: () => emitEditorCommand("print-preview"),
      },
      {
        id: "sheet-insert",
        title: "Insert Sheet…",
        mode: "sheet" as const,
        menuPath: ["Sheet", "Insert Sheet…"],
        action: () => emitEditorCommand("insert-sheet"),
      },
      {
        id: "sheet-delete",
        title: "Delete Sheet",
        mode: "sheet" as const,
        menuPath: ["Sheet", "Delete Sheet"],
        disabled: () => ctx.activeMode() !== "sheet" || (ctx.docContent()?.sheets?.length ?? 0) <= 1,
        action: () => emitEditorCommand("delete-sheet"),
      },
      {
        id: "sheet-sort-asc",
        title: "Sort Ascending",
        mode: "sheet" as const,
        menuPath: ["Sheet", "Sort Ascending"],
        separatorBefore: true,
        action: () => emitEditorCommand("sort-asc"),
      },
      {
        id: "sheet-sort-multi",
        title: "Sort…",
        mode: "sheet" as const,
        menuPath: ["Sheet", "Sort…"],
        action: () => emitEditorCommand("sort-multi"),
      },
      {
        id: "sheet-filter",
        title: "AutoFilter",
        mode: "sheet" as const,
        menuPath: ["Sheet", "AutoFilter"],
        action: () => emitEditorCommand("filter"),
      },
      {
        id: "slide-new",
        title: "New Slide",
        mode: "slide" as const,
        menuPath: ["Slide", "New Slide"],
        action: () => emitEditorCommand("new-slide"),
      },
      {
        id: "slide-delete",
        title: "Delete Slide",
        mode: "slide" as const,
        menuPath: ["Slide", "Delete Slide"],
        action: () => emitEditorCommand("delete-slide"),
      },
      {
        id: "slide-layout",
        title: "Layout…",
        mode: "slide" as const,
        menuPath: ["Slide", "Layout…"],
        separatorBefore: true,
        action: () => emitEditorCommand("slide-layout"),
      },
      {
        id: "slide-present",
        title: "Start from Beginning",
        mode: "slide" as const,
        menuPath: ["Slide", "Start from Beginning"],
        shortcut: "Shift+F5",
        action: () => emitEditorCommand("present"),
      },
      {
        id: "table-insert",
        title: "Insert Table…",
        mode: "doc" as const,
        menuPath: ["Table", "Insert Table…"],
        disabled: () => ctx.activeMode() !== "doc",
        action: () => emitEditorCommand("insert-table"),
      },
      {
        id: "word-count",
        title: "Word Count",
        mode: "doc" as const,
        menuPath: ["Tools", "Word Count"],
        disabled: () => ctx.activeMode() !== "doc",
        action: () => ctx.statusInfo() && showToast(ctx.statusInfo(), "success"),
      },
      {
        id: "options",
        title: "Options…",
        mode: "global" as const,
        menuPath: ["Tools", "Options…"],
        action: () => ctx.setSettingsOpen(true),
      },
      {
        id: "window-home",
        title: "Home",
        mode: "global" as const,
        menuPath: ["Window", "Home"],
        action: () => void ctx.goHome(),
      },
      {
        id: "window-doc",
        title: "Document",
        shortcut: "Ctrl+Alt+1",
        mode: "global" as const,
        menuPath: ["Window", "Document"],
        separatorBefore: true,
        action: () => void ctx.handleSwitchMode("doc"),
      },
      {
        id: "window-sheet",
        title: "Spreadsheet",
        shortcut: "Ctrl+Alt+2",
        mode: "global" as const,
        menuPath: ["Window", "Spreadsheet"],
        action: () => void ctx.handleSwitchMode("sheet"),
      },
      {
        id: "window-slide",
        title: "Presentation",
        shortcut: "Ctrl+Alt+3",
        mode: "global" as const,
        menuPath: ["Window", "Presentation"],
        action: () => void ctx.handleSwitchMode("slide"),
      },
      {
        id: "help-shortcuts",
        title: "Keyboard Shortcuts",
        shortcut: "F1",
        mode: "global" as const,
        menuPath: ["Help", "Keyboard Shortcuts"],
        action: () => ctx.setHelpOpen(true),
      },
      {
        id: "help-about",
        title: "About Redoc",
        mode: "global" as const,
        menuPath: ["Help", "About Redoc"],
        action: () => ctx.setAboutOpen(true),
      },
      {
        id: "help-command-palette",
        title: "Command Palette",
        shortcut: "Ctrl+K",
        mode: "global" as const,
        menuPath: ["Help", "Command Palette"],
        action: () => ctx.setPaletteOpen(true),
      },
    ];
}

export const FILE_MENU_ORDER = [
  "new-document",
  "new-spreadsheet",
  "new-presentation",
  "open-document",
  "save-document",
  "save-as-document",
  "print",
  "export-as",
  "go-home",
] as const;

export function reorderFileMenuCommands(registry: {
  get: (id: string) => CommandItem | undefined;
  unregister: (id: string) => void;
  register: (cmd: CommandItem) => void;
}) {
  for (const id of FILE_MENU_ORDER) {
    const cmd = registry.get(id);
    if (!cmd) continue;
    registry.unregister(id);
    registry.register({
      ...cmd,
      separatorBefore: id === "open-document" || id === "export-as" || id === "go-home",
    });
  }
}
