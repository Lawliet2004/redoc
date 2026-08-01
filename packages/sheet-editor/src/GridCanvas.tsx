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
import type { GridCanvasProps } from "./sheetTypes";

export function GridCanvas(props: GridCanvasProps) {
  const { containerRef, canvasRef, scrollTop, scrollLeft, setScrollTop, setScrollLeft, rowHeight, columnWidth, drawGrid, markGridDirtyFull, activeCell, redo, undo, copySelection, cutSelection, pasteValuesOnly, pasteTsv, selectCell, lastUsedCell, jumpToDataEdge, setEditing, setFormulaValue, formulaInputRef, handleCanvasClick, setContextMenu, emitEditorCommand, isFillHandlePoint, setFillDragStart, setSuppressNextClick, startDimensionDrag, updateDimensionDrag, fillDragStart, finishFillDrag, dimensionDrag, setDimensionDrag, getColName, cellsData, cellHyperlink, chartType, chartData, chartMax, pieSlices } = props;
  return (
    <>
      {/* Grid Container */}
        <div
          ref={containerRef}
          tabindex="0"
          onWheel={(event) => {
            event.preventDefault();
            markGridDirtyFull?.();
            setScrollTop(Math.max(0, Math.min(100000 * rowHeight(), scrollTop() + event.deltaY)));
            setScrollLeft(Math.max(0, Math.min(1000 * columnWidth(), scrollLeft() + event.deltaX)));
            drawGrid();
          }}
          onKeyDown={(event) => {
            const cell = activeCell();
            if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "z") {
              void (event.shiftKey ? redo() : undo());
              event.preventDefault();
              return;
            }
            if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "y") {
              void redo();
              event.preventDefault();
              return;
            }
            if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "c") {
              void copySelection();
              event.preventDefault();
              return;
            }
            if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "x") {
              void cutSelection();
              event.preventDefault();
              return;
            }
            if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "v") {
              if (event.shiftKey) {
                void pasteValuesOnly();
                event.preventDefault();
                event.stopPropagation();
                return;
              }
              void pasteTsv();
              event.preventDefault();
              return;
            }
            if ((event.ctrlKey || event.metaKey) && event.key === "Home") {
              selectCell(1, 1, event.shiftKey);
              event.preventDefault();
              event.stopPropagation();
              return;
            }
            if ((event.ctrlKey || event.metaKey) && event.key === "End") {
              const last = lastUsedCell();
              selectCell(last.row, last.col, event.shiftKey);
              event.preventDefault();
              event.stopPropagation();
              return;
            }
            if ((event.ctrlKey || event.metaKey) && event.key === "ArrowUp") {
              jumpToDataEdge(-1, 0, event.shiftKey);
              event.preventDefault();
              event.stopPropagation();
              return;
            }
            if ((event.ctrlKey || event.metaKey) && event.key === "ArrowDown") {
              jumpToDataEdge(1, 0, event.shiftKey);
              event.preventDefault();
              event.stopPropagation();
              return;
            }
            if ((event.ctrlKey || event.metaKey) && event.key === "ArrowLeft") {
              jumpToDataEdge(0, -1, event.shiftKey);
              event.preventDefault();
              event.stopPropagation();
              return;
            }
            if ((event.ctrlKey || event.metaKey) && event.key === "ArrowRight") {
              jumpToDataEdge(0, 1, event.shiftKey);
              event.preventDefault();
              event.stopPropagation();
              return;
            }
            if (event.key === "F2" || event.key === "Enter") {
              setEditing(true);
              formulaInputRef?.focus();
              formulaInputRef?.select();
              event.preventDefault();
              return;
            }
            if (event.key === "ArrowUp") selectCell(cell.row - 1, cell.col, event.shiftKey);
            else if (event.key === "ArrowDown" || event.key === "Enter") selectCell(cell.row + 1, cell.col, event.shiftKey);
            else if (event.key === "ArrowLeft") selectCell(cell.row, cell.col - 1, event.shiftKey);
            else if (event.key === "ArrowRight" || event.key === "Tab") selectCell(cell.row, cell.col + 1, event.shiftKey);
            else if (event.key.length === 1 && !event.ctrlKey && !event.metaKey) {
              setEditing(true);
              setFormulaValue(event.key);
              formulaInputRef?.focus();
              formulaInputRef?.select();
              return;
            }
            else return;
            event.preventDefault();
          }}
          style={{ flex: 1, position: "relative", overflow: "hidden", background: "#ffffff" }}
        >
          <canvas
            ref={canvasRef}
            role="grid"
            aria-label="Spreadsheet grid"
            aria-rowcount="100000"
            aria-colcount="1000"
            aria-rowindex={activeCell().row}
            aria-colindex={activeCell().col}
            onClick={(event) => {
              if ((event.ctrlKey || event.metaKey) && cellHyperlink) {
                const cell = cellsData()[`${activeCell().row}:${activeCell().col}`];
                const href = cellHyperlink(cell);
                if (href) {
                  window.open(href, "_blank", "noopener,noreferrer");
                  event.preventDefault();
                  return;
                }
              }
              handleCanvasClick(event);
            }}
            onContextMenu={(event) => {
              event.preventDefault();
              const items: ContextMenuItem[] = [
                { id: "cut", label: "Cut", action: () => void cutSelection() },
                { id: "copy", label: "Copy", action: () => void copySelection() },
                { id: "paste", label: "Paste", action: () => void pasteTsv() },
                { id: "paste-values", label: "Paste Values", action: () => void pasteValuesOnly() },
                { id: "sep1", label: "", separator: true },
                { id: "sort-asc", label: "Sort Ascending", action: () => emitEditorCommand("sort-asc") },
                { id: "sort-multi", label: "Sort…", action: () => emitEditorCommand("sort-multi") },
                { id: "filter", label: "AutoFilter", action: () => emitEditorCommand("filter") },
                { id: "sep2", label: "", separator: true },
                { id: "insert-rows", label: "Insert Rows", action: () => emitEditorCommand("insert-rows") },
                { id: "delete-rows", label: "Delete Rows", action: () => emitEditorCommand("delete-rows") },
                { id: "insert-cols", label: "Insert Columns", action: () => emitEditorCommand("insert-cols") },
                { id: "delete-cols", label: "Delete Columns", action: () => emitEditorCommand("delete-cols") },
                { id: "freeze-from-selection", label: "Freeze Panes", action: () => emitEditorCommand("freeze-from-selection") },
                { id: "sep3", label: "", separator: true },
                {
                  id: "format-hint",
                  label: "Format: Bold / Currency / Align via toolbar",
                  disabled: true,
                },
              ];
              setContextMenu({ x: event.clientX, y: event.clientY, items });
            }}
            onPointerDown={(event) => {
              if (isFillHandlePoint(event)) {
                event.currentTarget.setPointerCapture(event.pointerId);
                setFillDragStart(activeCell());
                setSuppressNextClick(true);
                event.preventDefault();
              } else if (startDimensionDrag(event)) {
                event.currentTarget.setPointerCapture(event.pointerId);
                setSuppressNextClick(true);
                event.preventDefault();
              }
            }}
            onPointerMove={updateDimensionDrag}
            onPointerUp={(event) => {
              if (fillDragStart()) {
                event.currentTarget.releasePointerCapture(event.pointerId);
                void finishFillDrag(event);
                event.preventDefault();
              } else if (dimensionDrag()) {
                event.currentTarget.releasePointerCapture(event.pointerId);
                setDimensionDrag(null);
                event.preventDefault();
              }
            }}
            style={{ display: "block" }}
          />
          <div
            aria-live="polite"
            aria-atomic="true"
            style={{
              position: "absolute",
              width: "1px",
              height: "1px",
              padding: 0,
              margin: "-1px",
              overflow: "hidden",
              clip: "rect(0, 0, 0, 0)",
              "white-space": "nowrap",
              border: 0,
            }}
          >
            Cell {getColName(activeCell().col)}{activeCell().row}: {cellsData()[`${activeCell().row}:${activeCell().col}`]?.display || "empty"}
          </div>
          <Show when={chartType() && chartData().length}>
            <svg
              width="380"
              height="230"
              role="img"
              aria-label={`${chartType()} chart from selected range`}
              style={{ position: "absolute", top: "42px", left: "56px", background: "white", border: "1px solid var(--border-color)", "border-radius": "6px", "box-shadow": "var(--shadow-md)" }}
            >
              <Show when={chartType() === "bar"}>
                <For each={chartData()}>
                  {(item: any, index: any) => {
                    const barWidth = 340 / Math.max(1, chartData().length) - 4;
                    const barHeight = Math.abs(item.value) / chartMax() * 170;
                    return <rect x={20 + index() * (barWidth + 4)} y={190 - barHeight} width={barWidth} height={barHeight} fill="#3b82f6" />;
                  }}
                </For>
              </Show>
              <Show when={chartType() === "line"}>
                <polyline
                  fill="none"
                  stroke="#16a34a"
                  stroke-width="3"
                  points={chartData().map((item: any, index: any) => `${20 + index * (340 / Math.max(1, chartData().length - 1)), 190 - Math.abs(item.value) / chartMax() * 170}`).join(" ")}
                />
              </Show>
              <Show when={chartType() === "pie"}>
                <For each={pieSlices()}>
                  {(slice) => (
                    <Show
                      when={!slice.isFullCircle}
                      fallback={<circle cx="190" cy="115" r="80" fill={slice.color} />}
                    >
                      <path d={slice.pathData} fill={slice.color} />
                    </Show>
                  )}
                </For>
              </Show>
              <line x1="15" y1="190" x2="365" y2="190" stroke="#94a3b8" />
            </svg>
          </Show>
        </div>
        
    </>
  );
}
