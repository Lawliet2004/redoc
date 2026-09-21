import { createSignal, For, Show } from "solid-js";
import {
  ToolbarRow, ToolbarButton, ToolbarSep, ToolbarSelect, ToolbarColor
} from "@redoc/editor-common";
import { t } from "@redoc/ui";
import {
  IconBold, IconItalic, IconUnderline, IconStrikethrough, IconAlignLeft, IconAlignCenter,
  IconAlignRight, IconAlignJustify, IconList, IconOrderedList, IconUndo, IconRedo, IconSearch,
  IconTable, IconImage, IconLink, IconClearFormat, IconSuperscript, IconSubscript,
  IconTextColor, IconHighlight, IconLineSpacing, IconIndent, IconOutdent, IconPrint,
} from "@redoc/icons";
import type { DocToolbarProps } from "./docToolbarTypes";

const LINE_SPACINGS = ["1.0", "1.15", "1.5", "2.0", "3.0"];
const SPACINGS = [0, 3, 6, 12, 18, 24];

export type { DocToolbarProps } from "./docToolbarTypes";

/** Comment glyph — lucide "message-square" drawn in the shared stroke style. */
function IconComment() {
  return (
    <svg
      width="16"
      height="16"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="2"
      stroke-linecap="round"
      stroke-linejoin="round"
      aria-hidden="true"
    >
      <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
    </svg>
  );
}

/**
 * Single-row formatting toolbar with grouped controls: history & tools,
 * style/font family/size, inline marks, paragraph alignment, lists & spacing,
 * inserts — everything else collapses into the overflow menu (role=menu).
 * Roving tabindex comes from ToolbarRow.
 */
export function DocToolbar(props: DocToolbarProps) {
  const [overflowOpen, setOverflowOpen] = createSignal(false);

  const overflowGroups: Array<{ label: string; action: () => void }> = [
    { label: "New", action: () => props.onRequestNew?.() },
    { label: "Open", action: () => props.onRequestOpen?.() },
    { label: "Save", action: () => props.onRequestSave?.() },
    { label: "Export PDF", action: () => props.onRequestExportPdf?.() },
    { label: "Export DOCX", action: () => props.onRequestExportDocx?.() },
    { label: "Print", action: () => window.print() },
    { label: "Page Setup", action: props.onPageSetup },
    { label: "Cut", action: props.onCut },
    { label: "Copy", action: props.onCopy },
    { label: "Paste", action: props.onPaste },
    { label: "Insert Bookmark", action: props.onInsertBookmark },
    { label: "Insert Page Break", action: props.onInsertPageBreak },
    { label: "Insert Page Number Field", action: props.onInsertPageField },
    { label: "Insert Total Pages Field", action: props.onInsertNumPagesField },
    { label: "Insert Section Break", action: props.onInsertSectionBreak },
    { label: "Insert Table of Contents", action: props.onInsertToc },
    { label: "Add table row", action: props.onAddTableRow },
    { label: "Delete table row", action: props.onDeleteTableRow },
    { label: "Add table column", action: props.onAddTableColumn },
    { label: "Delete table column", action: props.onDeleteTableColumn },
    { label: "Delete table", action: props.onDeleteTable },
    { label: "Merge cells", action: props.onMergeCells },
    { label: "Split cell", action: props.onSplitCell },
    { label: "Toggle header row", action: props.onToggleHeaderRow },
    { label: "Format Painter", action: props.onToggleFormatPainter },
  ];

  const spacingSelect = (label: string, value: string, onChange: (v: string) => void) => (
    <ToolbarSelect
      ariaLabel={label}
      width="64px"
      value={value}
      onChange={onChange}
      options={SPACINGS.map((s) => ({ value: String(s), label: String(s) }))}
    />
  );

  const marks = () => props.activeMarks ?? {};

  return (
    <ToolbarRow class="doc-toolbar">
      {/* History + tools */}
      <ToolbarButton title="Undo (Ctrl+Z)" onClick={props.onUndo}><IconUndo /></ToolbarButton>
      <ToolbarButton title="Redo (Ctrl+Y)" onClick={props.onRedo}><IconRedo /></ToolbarButton>
      <ToolbarButton title="Print preview" onClick={() => props.onPrintPreview?.()}><IconPrint /></ToolbarButton>
      <ToolbarButton title="Find and Replace (Ctrl+F)" onClick={props.onToggleFind} active={props.findOpen}><IconSearch /></ToolbarButton>
      <ToolbarSep />

      {/* Block style + font */}
      <ToolbarSelect
        ariaLabel="Paragraph style"
        width="140px"
        value={props.currentBlockType}
        onChange={props.onChangeBlockType}
        options={[
          { value: "paragraph", label: "Default Paragraph Style" },
          { value: "heading1", label: "Heading 1" },
          { value: "heading2", label: "Heading 2" },
          { value: "heading3", label: "Heading 3" },
          { value: "heading4", label: "Heading 4" },
          { value: "heading5", label: "Heading 5" },
          { value: "heading6", label: "Heading 6" },
          { value: "blockquote", label: "Block Quote" },
          { value: "code_block", label: "Preformatted Text" },
        ]}
      />
      <ToolbarSelect
        ariaLabel="Font"
        width="110px"
        value={props.fontFamily}
        onChange={props.onChangeFontFamily}
        options={[
          { value: "Liberation Serif", label: "Liberation Serif" },
          { value: "Liberation Sans", label: "Liberation Sans" },
          { value: "Times New Roman", label: "Times New Roman" },
          { value: "Arial", label: "Arial" },
          { value: "Georgia", label: "Georgia" },
          { value: "Courier New", label: "Courier New" },
        ]}
      />
      <ToolbarButton title="Decrease font size (Ctrl+[)" onClick={props.onShrinkFontSize}>
        <span class="doc-toolbar-fontsize-btn" aria-hidden="true">A−</span>
      </ToolbarButton>
      <ToolbarSelect
        ariaLabel="Font size"
        width="52px"
        value={props.fontSize}
        onChange={props.onChangeFontSize}
        options={["8", "9", "10", "11", "12", "14", "16", "18", "20", "24", "28", "36"].map((s) => ({
          value: s,
          label: s,
        }))}
      />
      <ToolbarButton title="Increase font size (Ctrl+])" onClick={props.onGrowFontSize}>
        <span class="doc-toolbar-fontsize-btn" aria-hidden="true">A+</span>
      </ToolbarButton>
      <ToolbarSep />

      {/* Inline marks */}
      <ToolbarButton title="Bold (Ctrl+B)" active={marks().bold} onClick={props.onToggleBold}><IconBold /></ToolbarButton>
      <ToolbarButton title="Italic (Ctrl+I)" active={marks().italic} onClick={props.onToggleItalic}><IconItalic /></ToolbarButton>
      <ToolbarButton title="Underline (Ctrl+U)" active={marks().underline} onClick={props.onToggleUnderline}><IconUnderline /></ToolbarButton>
      <ToolbarButton title="Strikethrough" active={marks().strike} onClick={props.onToggleStrike}><IconStrikethrough /></ToolbarButton>
      <ToolbarButton title="Superscript (Ctrl+.)" onClick={props.onToggleSuper}><IconSuperscript /></ToolbarButton>
      <ToolbarButton title="Subscript (Ctrl+,)" onClick={props.onToggleSub}><IconSubscript /></ToolbarButton>
      <ToolbarColor title="Text Color" value={props.textColor} onChange={props.onChangeTextColor}><IconTextColor /></ToolbarColor>
      <ToolbarColor title="Highlight Color" value={props.highlightColor} onChange={props.onChangeHighlightColor}><IconHighlight /></ToolbarColor>
      <ToolbarButton title="Clear Direct Formatting" onClick={props.onClearFormatting}><IconClearFormat /></ToolbarButton>
      <ToolbarSep />

      {/* Paragraph alignment */}
      <ToolbarButton title="Align Left" onClick={props.onAlignLeft}><IconAlignLeft /></ToolbarButton>
      <ToolbarButton title="Align Center" onClick={props.onAlignCenter}><IconAlignCenter /></ToolbarButton>
      <ToolbarButton title="Align Right" onClick={props.onAlignRight}><IconAlignRight /></ToolbarButton>
      <ToolbarButton title="Justify" onClick={props.onAlignJustify}><IconAlignJustify /></ToolbarButton>
      <ToolbarButton title="Decrease Indent" onClick={props.onDecreaseIndent}><IconOutdent /></ToolbarButton>
      <ToolbarButton title="Increase Indent" onClick={props.onIncreaseIndent}><IconIndent /></ToolbarButton>
      <ToolbarSep />

      {/* Lists + spacing */}
      <ToolbarButton title="Bulleted list" active={props.bulletListActive} onClick={props.onListBullet}><IconList /></ToolbarButton>
      <ToolbarButton title="Numbered list" active={props.orderedListActive} onClick={props.onListOrdered}><IconOrderedList /></ToolbarButton>
      <ToolbarSelect
        ariaLabel="Line spacing"
        width="64px"
        value={props.currentLineSpacing}
        onChange={(v) => props.onChangeLineSpacing(Number(v))}
        options={LINE_SPACINGS.map((s) => ({ value: s, label: s }))}
      />
      <ToolbarSep />

      {/* Inserts */}
      <ToolbarButton title="Insert Table" onClick={props.onInsertTable}><IconTable /></ToolbarButton>
      <ToolbarButton title="Insert Image" onClick={props.onInsertImage}><IconImage /></ToolbarButton>
      <ToolbarButton title="Insert Link" active={marks().link} onClick={props.onInsertLink}><IconLink /></ToolbarButton>
      <ToolbarButton title="Add Comment" onClick={() => props.onAddComment?.()}><IconComment /></ToolbarButton>

      {/* Overflow: everything else lives in one menu — single visible row */}
      <div style={{ position: "relative" }}>
        <ToolbarButton
          title={t("toolbar.moreActions")}
          ariaLabel={t("toolbar.moreActions")}
          active={overflowOpen()}
          onClick={() => setOverflowOpen(!overflowOpen())}
        >
          ⋯
        </ToolbarButton>
        <Show when={overflowOpen()}>
          <div role="menu" aria-label={t("toolbar.overflowLabel")} class="g-cmd-menu g-cmd-overflow">
            <div class="g-cmd-menuitem" role="none" style={{ display: "flex", "align-items": "center", gap: "8px", padding: "6px 12px" }}>
              <span><IconLineSpacing /></span>
              <label style={{ display: "flex", "align-items": "center", gap: "6px", "font-size": "13px" }}>
                Spacing before
                {spacingSelect("Spacing before", String(props.spacingBefore), (v) => props.onChangeSpacingBefore(Number(v)))}
              </label>
              <label style={{ display: "flex", "align-items": "center", gap: "6px", "font-size": "13px" }}>
                after
                {spacingSelect("Spacing after", String(props.spacingAfter), (v) => props.onChangeSpacingAfter(Number(v)))}
              </label>
            </div>
            <div class="g-cmd-sep" role="separator" />
            <For each={overflowGroups}>
              {(item) => (
                <button
                  type="button"
                  role="menuitem"
                  class="g-cmd-menuitem"
                  onClick={() => {
                    item.action();
                    setOverflowOpen(false);
                  }}
                >
                  <span>{item.label}</span>
                </button>
              )}
            </For>
          </div>
        </Show>
      </div>
    </ToolbarRow>
  );
}
