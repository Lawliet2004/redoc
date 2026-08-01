import { createSignal, onCleanup, onMount, For, Show } from "solid-js";
import {
  IconPlus, IconBold, IconItalic, IconUnderline, IconTextColor, IconHighlight,
  IconAlignLeft, IconAlignCenter, IconAlignRight, IconAlignTop, IconAlignMiddle, IconAlignBottom,
  IconUndo, IconRedo, IconCut, IconCopy, IconPaste, IconNew, IconFolderOpen, IconSave, IconPdf,
  IconPrint, IconSortAsc, IconSortDesc, IconFilter, IconChart, IconImage, IconCurrency, IconPercent,
  IconMerge, IconWrap, IconSum, IconEquals, IconFreeze, IconProperties, IconStyles, IconGallery,
  IconNavigator, IconFunctions,
} from "@redoc/icons";
import {
  ToolbarRow, ToolbarButton, ToolbarSep, ToolbarSelect, ToolbarColor,
  FindBar, IconSidebar, type SidebarPanel,
  ContextMenu, type ContextMenuItem,
  EDITOR_COMMAND, type EditorCommandDetail, emitEditorCommand,
} from "@redoc/editor-common";

export function FormulaBar(props: any) {
  const { activeCell, formulaValue, setFormulaValue, getColName, insertFormulaPrefix, commitCellEdit, editing, setEditing, cellsData, containerRef, formulaInputRef } = props;
  return (
    <>
      {/* Formula bar */}
      <div class="g-formula-bar g-no-print">
        <div
          style={{
            width: "72px",
            "border-right": "1px solid var(--border-color)",
            display: "flex",
            "align-items": "center",
            "justify-content": "center",
            "font-size": "12px",
            "font-family": "var(--font-sans)",
            color: "var(--text-primary)",
            "flex-shrink": "0",
          }}
        >
          {getColName(activeCell().col)}{activeCell().row}
        </div>
        <ToolbarButton title="Function wizard" onClick={() => formulaInputRef?.focus()}>fx</ToolbarButton>
        <ToolbarButton title="Sum" onClick={() => insertFormulaPrefix("=SUM()")}><IconSum /></ToolbarButton>
        <ToolbarButton title="Equals" onClick={() => insertFormulaPrefix("=")}><IconEquals /></ToolbarButton>
        <div style={{ width: "1px", background: "var(--border-color)", "align-self": "stretch" }} />
        <input
          ref={formulaInputRef}
          type="text"
          aria-label="Formula input"
          value={formulaValue()}
          onFocus={() => setEditing(true)}
          onInput={(e) => {
            setFormulaValue(e.currentTarget.value);
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              void commitCellEdit(formulaValue());
              containerRef?.focus();
            } else if (e.key === "Escape") {
              e.preventDefault();
              setFormulaValue(cellsData()[`${activeCell().row}:${activeCell().col}`]?.raw || "");
              containerRef?.focus();
            }
          }}
          onBlur={() => {
            if (editing()) void commitCellEdit(formulaValue());
          }}
          style={{
            flex: 1,
            border: "none",
            background: "transparent",
            color: "var(--text-primary)",
            "font-family": "var(--font-sans)",
            "font-size": "12px",
            padding: "0 8px",
            outline: "none",
          }}
        />
      </div>

      
    </>
  );
}
