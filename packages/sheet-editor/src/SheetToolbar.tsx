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
import type { SheetToolbarProps } from "./sheetTypes";

export function SheetToolbar(props: SheetToolbarProps) {
  const { fontFamily, setFontFamily, fontSize, setFontSize, activeStyle, updateActiveStyle, selectedBounds, activeCell, setMerges, pushHistory, makeWorkbook, cellsData, drawGrid } = props;
  const [borderStyle, setBorderStyle] = createSignal("thin");
  const [borderColor, setBorderColor] = createSignal("#0f172a");
  return (
    <>
      {/* Formatting toolbar — font | emphasis | colors | alignment | numbers | borders */}
      <ToolbarRow class="sheet-toolbar sheet-toolbar-format">
        <ToolbarSelect
          ariaLabel="Font"
          width="120px"
          value={fontFamily()}
          onChange={(v) => {
            setFontFamily(v);
            updateActiveStyle({ fontFamily: v });
          }}
          options={[
            { value: "Liberation Sans", label: "Liberation Sans" },
            { value: "Liberation Serif", label: "Liberation Serif" },
            { value: "Liberation Mono", label: "Liberation Mono" },
            { value: "Arial", label: "Arial" },
            { value: "Calibri", label: "Calibri" },
            { value: "Courier New", label: "Courier New" },
          ]}
        />
        <ToolbarSelect
          ariaLabel="Font size"
          width="52px"
          value={fontSize()}
          onChange={(v) => {
            setFontSize(v);
            updateActiveStyle({ fontSize: Number(v) || 12 });
          }}
          options={["8", "9", "10", "11", "12", "14", "16", "18", "20", "24"].map((s) => ({ value: s, label: s }))}
        />
        <ToolbarSep />
        <ToolbarButton
          title="Bold"
          active={!!activeStyle()?.bold}
          onClick={() => updateActiveStyle({ bold: !activeStyle()?.bold })}
        >
          <IconBold />
        </ToolbarButton>
        <ToolbarButton
          title="Italic"
          active={!!activeStyle()?.italic}
          onClick={() => updateActiveStyle({ italic: !activeStyle()?.italic })}
        >
          <IconItalic />
        </ToolbarButton>
        <ToolbarButton
          title="Underline"
          active={!!activeStyle()?.underline}
          onClick={() => updateActiveStyle({ underline: !activeStyle()?.underline })}
        >
          <IconUnderline />
        </ToolbarButton>
        <ToolbarSep />
        <ToolbarColor
          title="Font Color"
          value={activeStyle()?.fontColor || "#0f172a"}
          onChange={(color) => updateActiveStyle({ fontColor: color })}
        >
          <IconTextColor />
          <span style={{ width: "14px", height: "3px", background: activeStyle()?.fontColor || "#0f172a", display: "block", "margin-top": "-2px" }} />
        </ToolbarColor>
        <ToolbarColor
          title="Background Color"
          value={activeStyle()?.bgColor || "#ffffff"}
          onChange={(color) => updateActiveStyle({ bgColor: color })}
        >
          <IconHighlight />
          <span style={{ width: "14px", height: "3px", background: activeStyle()?.bgColor || "#ffffff", display: "block", "margin-top": "-2px" }} />
        </ToolbarColor>
        <ToolbarSep />
        <ToolbarButton title="Align Left" active={activeStyle()?.align === "left"} onClick={() => updateActiveStyle({ align: "left" })}><IconAlignLeft /></ToolbarButton>
        <ToolbarButton title="Align Center" active={activeStyle()?.align === "center"} onClick={() => updateActiveStyle({ align: "center" })}><IconAlignCenter /></ToolbarButton>
        <ToolbarButton title="Align Right" active={activeStyle()?.align === "right"} onClick={() => updateActiveStyle({ align: "right" })}><IconAlignRight /></ToolbarButton>
        <ToolbarSep />
        <ToolbarButton title="Align Top" active={activeStyle()?.vAlign === "top"} onClick={() => updateActiveStyle({ vAlign: "top" })}><IconAlignTop /></ToolbarButton>
        <ToolbarButton title="Align Middle" active={activeStyle()?.vAlign === "middle"} onClick={() => updateActiveStyle({ vAlign: "middle" })}><IconAlignMiddle /></ToolbarButton>
        <ToolbarButton title="Align Bottom" active={activeStyle()?.vAlign === "bottom"} onClick={() => updateActiveStyle({ vAlign: "bottom" })}><IconAlignBottom /></ToolbarButton>
        <ToolbarSep />
        <ToolbarButton
          title="Wrap text"
          active={!!activeStyle()?.wrap}
          onClick={() => updateActiveStyle({ wrap: !activeStyle()?.wrap })}
        >
          <IconWrap />
        </ToolbarButton>
        <ToolbarButton
          title="Merge cells"
          onClick={() => {
            const bounds = selectedBounds();
            if (bounds.startRow === bounds.endRow && bounds.startCol === bounds.endCol) return;
            const next: any = {
              startRow: bounds.startRow,
              endRow: bounds.endRow,
              startCol: bounds.startCol,
              endCol: bounds.endCol,
            };
            setMerges((prev: any) => {
              const filtered = prev.filter(
                (m: any) =>
                  m.endRow < next.startRow ||
                  m.startRow > next.endRow ||
                  m.endCol < next.startCol ||
                  m.startCol > next.endCol
              );
              return [...filtered, next];
            });
            pushHistory();
            queueMicrotask(() => props.onChange?.(makeWorkbook(cellsData())));
            drawGrid();
          }}
        >
          <IconMerge />
        </ToolbarButton>
        <ToolbarButton
          title="Unmerge cells"
          onClick={() => {
            const { row, col } = activeCell();
            setMerges((prev: any) =>
              prev.filter(
                (m: any) => !(row >= m.startRow && row <= m.endRow && col >= m.startCol && col <= m.endCol)
              )
            );
            queueMicrotask(() => props.onChange?.(makeWorkbook(cellsData())));
            drawGrid();
          }}
        >
          ⌗
        </ToolbarButton>
        <ToolbarSep />
        <ToolbarButton title="Currency" active={activeStyle()?.format === "currency"} onClick={() => updateActiveStyle({ format: "currency" })}><IconCurrency /></ToolbarButton>
        <ToolbarButton title="Percent" active={activeStyle()?.format === "percent"} onClick={() => updateActiveStyle({ format: "percent" })}><IconPercent /></ToolbarButton>
        <ToolbarButton title="General number format" active={!activeStyle()?.format || activeStyle()?.format === "general"} onClick={() => updateActiveStyle({ format: "general" })}>123</ToolbarButton>
        <ToolbarButton title="Increase decimals" onClick={() => updateActiveStyle({ decimals: Math.min((activeStyle()?.decimals ?? 2) + 1, 10) })}>.0</ToolbarButton>
        <ToolbarButton title="Decrease decimals" onClick={() => updateActiveStyle({ decimals: Math.max((activeStyle()?.decimals ?? 2) - 1, 0) })}>.00</ToolbarButton>
        <ToolbarSep />
        <ToolbarButton title="Number format…" onClick={() => props.onOpenNumberFormat?.()}>Format…</ToolbarButton>
        <ToolbarButton title="Data validation list…" onClick={() => props.onOpenValidation?.()}>List ▾</ToolbarButton>
        <ToolbarSep />
        <ToolbarSelect
          ariaLabel="Border style"
          width="90px"
          value={borderStyle()}
          onChange={(v) => setBorderStyle(v)}
          options={[
            { value: "thin", label: "Thin" },
            { value: "medium", label: "Medium" },
            { value: "thick", label: "Thick" },
            { value: "dashed", label: "Dashed" },
            { value: "dotted", label: "Dotted" },
            { value: "double", label: "Double" },
          ]}
        />
        <ToolbarColor
          title="Border color"
          value={borderColor()}
          onChange={(color) => setBorderColor(color)}
        >
          <span style={{ width: "14px", height: "14px", border: "2px solid currentColor", display: "block", "border-radius": "2px" }} />
        </ToolbarColor>
        <ToolbarSelect
          ariaLabel="Apply borders"
          width="110px"
          value=""
          onChange={(v) => {
            if (v) props.onApplyBorders?.(v as any, borderStyle(), borderColor());
          }}
          options={[
            { value: "", label: "Borders…" },
            { value: "all", label: "All borders" },
            { value: "outer", label: "Outer border" },
            { value: "top", label: "Top border" },
            { value: "bottom", label: "Bottom border" },
            { value: "none", label: "No borders" },
          ]}
        />
      </ToolbarRow>

      
    </>
  );
}
