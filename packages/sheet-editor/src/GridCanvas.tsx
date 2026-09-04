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
  const { containerRef, canvasRef, scrollTop, scrollLeft, setScrollTop, setScrollLeft, rowHeight, columnWidth, drawGrid, markGridDirtyFull, activeCell, redo, undo, copySelection, cutSelection, pasteValuesOnly, pasteTsv, selectCell, lastUsedCell, jumpToDataEdge, setEditing, setFormulaValue, formulaInputRef, handleCanvasClick, setContextMenu, emitEditorCommand, isFillHandlePoint, setFillDragStart, setSuppressNextClick, startDimensionDrag, updateDimensionDrag, fillDragStart, finishFillDrag, dimensionDrag, setDimensionDrag, getColName, cellsData, cellHyperlink, onOpenHyperlink, chartType, chartTop, chartLeft, chartData, chartMax, pieSlices } = props;
  let scrollRafId: number | null = null;
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
            if (scrollRafId === null) {
              scrollRafId = requestAnimationFrame(() => {
                drawGrid();
                scrollRafId = null;
              });
            }
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
              if (event.ctrlKey || event.metaKey) {
                // Resolve the clicked cell before opening its link; activeCell may still refer
                // to the previous selection when the pointer lands on a different cell.
                handleCanvasClick(event);
                const cell = cellsData()[`${activeCell().row}:${activeCell().col}`];
                const href = cellHyperlink(cell);
                if (href) {
                  if (!onOpenHyperlink?.(href)) window.open(href, "_blank", "noopener,noreferrer");
                  event.preventDefault();
                  return;
                }
                return;
              }
              handleCanvasClick(event);
            }}
            onDblClick={(event) => {
              const cell = activeCell();
              setFormulaValue(cellsData()[`${cell.row}:${cell.col}`]?.raw || "");
              setEditing(true);
              formulaInputRef?.focus();
              formulaInputRef?.select();
            }}
            onContextMenu={(event) => {
              event.preventDefault();
              const items: ContextMenuItem[] = [
                { id: "cut", label: "Cut", action: () => void cutSelection() },
                { id: "copy", label: "Copy", action: () => void copySelection() },
                { id: "paste", label: "Paste", action: () => void pasteTsv() },
                { id: "sep1", label: "", separator: true },
                { id: "insert-row-above", label: "Insert Row Above", action: () => emitEditorCommand("insert-row-above") },
                { id: "insert-row-below", label: "Insert Row Below", action: () => emitEditorCommand("insert-row-below") },
                { id: "insert-col-left", label: "Insert Column Left", action: () => emitEditorCommand("insert-col-left") },
                { id: "insert-col-right", label: "Insert Column Right", action: () => emitEditorCommand("insert-col-right") },
                { id: "delete-row", label: "Delete Row", action: () => emitEditorCommand("delete-rows") },
                { id: "delete-col", label: "Delete Column", action: () => emitEditorCommand("delete-cols") },
                { id: "sep2", label: "", separator: true },
                { id: "format-cells", label: "Format Cells...", action: () => emitEditorCommand("format-cells") },
                { id: "clear-contents", label: "Clear Contents", action: () => emitEditorCommand("clear-contents") },
                { id: "clear-formatting", label: "Clear Formatting", action: () => emitEditorCommand("clear-formatting") },
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
              style={{ position: "absolute", top: `${chartTop()}px`, left: `${chartLeft()}px`, background: "white", border: "1px solid var(--border-color)", "border-radius": "6px", "box-shadow": "var(--shadow-md)" }}
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
              <Show when={chartType() === "area"}>
                <polygon
                  fill="#8b5cf6"
                  fill-opacity="0.35"
                  stroke="#8b5cf6"
                  stroke-width="2"
                  points={`20,190 ${chartData().map((item: any, index: any) => `${20 + index * (340 / Math.max(1, chartData().length - 1))},${190 - Math.abs(item.value) / chartMax() * 170}`).join(" ")} 365,190`}
                />
              </Show>
              <Show when={chartType() === "line"}>
                <polyline
                  fill="none"
                  stroke="#16a34a"
                  stroke-width="3"
                  points={chartData().map((item: any, index: any) => `${20 + index * (340 / Math.max(1, chartData().length - 1)), 190 - Math.abs(item.value) / chartMax() * 170}`).join(" ")}
                />
              </Show>
              <Show when={chartType() === "scatter"}>
                <For each={chartData()}>
                  {(item: any, index: any) => (
                    <circle
                      cx={20 + index() * (340 / Math.max(1, chartData().length - 1))}
                      cy={190 - Math.abs(item.value) / chartMax() * 170}
                      r="4"
                      fill="#ef4444"
                    />
                  )}
                </For>
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
              <Show when={chartType() === "doughnut"}>
                <For each={pieSlices()}>
                  {(slice) => (
                    <Show
                      when={!slice.isFullCircle}
                      fallback={<circle cx="190" cy="115" r="80" fill="none" stroke={slice.color} stroke-width="36" />}
                    >
                      <path
                        d={slice.pathData}
                        fill={slice.color}
                        fill-opacity="0.35"
                        stroke={slice.color}
                        stroke-width="18"
                      />
                    </Show>
                  )}
                </For>
                <circle cx="190" cy="115" r="62" fill="#ffffff" />
              </Show>
              <line x1="15" y1="190" x2="365" y2="190" stroke="#94a3b8" />
              <Show when={props.chartTitle && chartType() !== "pie" && chartType() !== "doughnut"}>
                <text x="190" y="14" text-anchor="middle" font-size="12" font-weight="600" fill="#0f172a">{props.chartTitle}</text>
                {/* Axis max label for the value scale. */}
                <text x="12" y="26" font-size="9" fill="#64748b">{Math.round(chartMax())}</text>
                <text x="12" y="192" font-size="9" fill="#64748b">0</text>
              </Show>
              <Show when={props.chartTitle && (chartType() === "pie" || chartType() === "doughnut")}>
                <text x="190" y="14" text-anchor="middle" font-size="12" font-weight="600" fill="#0f172a">{props.chartTitle}</text>
              </Show>
            </svg>
          </Show>
        </div>

    </>
  );
}
