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
import type { FormulaBarProps } from "./sheetTypes";
import { resolveValidationOptions } from "./validationOptions";

export function FormulaBar(props: FormulaBarProps) {
  const { activeCell, formulaValue, setFormulaValue, getColName, insertFormulaPrefix, commitCellEdit, editing, setEditing, cellsData, containerRef, formulaInputRef } = props;
  let inputEl: HTMLInputElement | undefined;
  const activeValidation = () => cellsData()[`${activeCell().row}:${activeCell().col}`]?.style?.validation;
  const validationOptions = () => resolveValidationOptions(
    activeValidation(),
    cellsData(),
    props.namedRanges?.() ?? [],
    props.activeSheetName?.() ?? "",
    props.cellsBySheet?.() ?? {},
  );
  return (
    <>
      {/* Formula bar — name box + fx affordances + input. Formulas render in
          the monospace font; literal values stay in the UI font. */}
      <div class="g-formula-bar sheet-formula-bar g-no-print">
        <div class="sheet-name-box" aria-label="Active cell reference" title="Active cell">
          {getColName(activeCell().col)}{activeCell().row}
        </div>
        <span class="sheet-fx">
          <ToolbarButton title="Function wizard" onClick={() => inputEl?.focus()}>fx</ToolbarButton>
        </span>
        <ToolbarButton title="Sum" onClick={() => insertFormulaPrefix("=SUM()")}><IconSum /></ToolbarButton>
        <ToolbarButton title="Equals" onClick={() => insertFormulaPrefix("=")}><IconEquals /></ToolbarButton>
        <Show when={activeValidation()?.type === "list" && validationOptions().length}>
          <select
            aria-label="Pick from list"
            class="g-toolbar-input sheet-validation-picker"
            value={formulaValue()}
            onChange={(e) => {
              const val = e.currentTarget.value;
              setFormulaValue(val);
              void commitCellEdit(val);
            }}
          >
            <option value="">—</option>
            <For each={validationOptions()}>{(opt) => <option value={opt}>{opt}</option>}</For>
          </select>
        </Show>
        <div style={{ width: "1px", background: "var(--border-color)", "align-self": "stretch" }} />
        <input
          ref={(el) => {
            inputEl = el;
            formulaInputRef?.(el);
          }}
          type="text"
          class="sheet-formula-input"
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
              containerRef?.()?.focus();
            } else if (e.key === "Escape") {
              e.preventDefault();
              setFormulaValue(cellsData()[`${activeCell().row}:${activeCell().col}`]?.raw || "");
              containerRef?.()?.focus();
            }
          }}
          onBlur={() => {
            if (editing()) void commitCellEdit(formulaValue());
          }}
          style={{
            "font-family": formulaValue().startsWith("=") ? "var(--font-mono)" : "var(--font-sans)",
          }}
        />
      </div>

      
    </>
  );
}
