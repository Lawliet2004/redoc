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
  const { containerRef, canvasRef, scrollTop, scrollLeft, setScrollTop, setScrollLeft, rowHeight, columnWidth, drawGrid, markGridDirtyFull, activeCell, redo, undo, copySelection, cutSelection, pasteValuesOnly, pasteTsv, selectCell, lastUsedCell, jumpToDataEdge, setEditing, setFormulaValue, formulaInputRef, handleCanvasClick, setContextMenu, emitEditorCommand, isFillHandlePoint, setFillDragStart, setSuppressNextClick, startDimensionDrag, updateDimensionDrag, commitDimensionDrag, fillDragStart, finishFillDrag, dimensionDrag, setDimensionDrag, getColName, cellsData, cellHyperlink, onOpenHyperlink, toggleHideRows, toggleHideCols, unhideCandidateRows, unhideCandidateCols, hiddenRows, hiddenCols, chartType, chartTop, chartLeft, chartData, chartMax, pieSlices, SERIES_COLORS, conditionalFormatting, clearSelectionContents, switchSheetByOffset } = props;
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
            if ((event.ctrlKey || event.metaKey) && (event.key === "PageUp" || event.key === "PageDown")) {
              switchSheetByOffset(event.key === "PageDown" ? 1 : -1);
              event.preventDefault();
              event.stopPropagation();
              return;
            }
            if (event.key === "Delete" || event.key === "Backspace") {
              void clearSelectionContents();
              event.preventDefault();
              return;
            }
            if (event.key === "PageUp" || event.key === "PageDown") {
              const direction = event.key === "PageDown" ? 1 : -1;
              const viewportRows = Math.max(1, Math.floor(((containerRef?.clientHeight || 400) - 26) / rowHeight()));
              const target = Math.max(1, Math.min(100000, activeCell().row + direction * viewportRows));
              selectCell(target, activeCell().col, event.shiftKey);
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
            tabindex="0"
            onFocus={() => {
              // Announce focus to screen readers
              const liveRegion = containerRef?.querySelector('[aria-live]');
              if (liveRegion) {
                liveRegion.textContent = `Spreadsheet grid focused. Use arrow keys to navigate cells.`;
              }
            }}
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
              const rect = canvasRef.getBoundingClientRect();
              const localX = event.clientX - rect.left;
              const localY = event.clientY - rect.top;
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
              // Header zones: row strip (left) / column strip (top).
              if (localX <= 40 && localY > 26) {
                const row = activeCell().row;
                items.push({ id: "sep3", label: "", separator: true });
                items.push({ id: "hide-rows", label: "Hide Row", action: () => toggleHideRows([row], true) });
                if (hiddenRows().length > 0) {
                  items.push({
                    id: "unhide-rows",
                    label: "Unhide Rows",
                    action: () => toggleHideRows(unhideCandidateRows(), false),
                  });
                }
              } else if (localY <= 26 && localX > 40) {
                const col = activeCell().col;
                items.push({ id: "sep3", label: "", separator: true });
                items.push({ id: "hide-cols", label: "Hide Column", action: () => toggleHideCols([col], true) });
                if (hiddenCols().length > 0) {
                  items.push({
                    id: "unhide-cols",
                    label: "Unhide Columns",
                    action: () => toggleHideCols(unhideCandidateCols(), false),
                  });
                }
              }
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
                commitDimensionDrag();
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
          <Show when={chartType() && chartData().series.length && chartData().labels.length}>
            <svg
              width="380"
              height="230"
              role="img"
              aria-label={`${chartType()} chart: ${props.chartTitle || "Chart"}`}
              aria-describedby={`chart-desc-${chartType()}`}
              style={{ position: "absolute", top: `${chartTop()}px`, left: `${chartLeft()}px`, background: "white", border: "1px solid var(--border-color)", "border-radius": "6px", "box-shadow": "var(--shadow-md)" }}
            >
              <Show when={chartType() === "bar"}>
                <For each={chartData().series}>
                  {(series: any, seriesIndex: () => number) => (
                    <For each={series.values}>
                      {(value: any, index: () => number) => {
                        const groupCount = chartData().series.length;
                        const slotWidth = 340 / Math.max(1, chartData().labels.length);
                        const barWidth = Math.max(1, slotWidth / (groupCount + 1) - 2);
                        const barHeight = Math.abs(value) / chartMax() * 170;
                        const groupOffset = (seriesIndex() - (groupCount - 1) / 2) * (barWidth + 1);
                        const x = 20 + index() * slotWidth + slotWidth / 2 + groupOffset - barWidth / 2;
                        return <rect x={x} y={190 - barHeight} width={barWidth} height={barHeight} fill={SERIES_COLORS[seriesIndex() % SERIES_COLORS.length]} />;
                      }}
                    </For>
                  )}
                </For>
              </Show>
              <Show when={chartType() === "area"}>
                <For each={chartData().series}>
                  {(series: any, seriesIndex: () => number) => (
                    <polygon
                      fill={SERIES_COLORS[seriesIndex() % SERIES_COLORS.length]}
                      fill-opacity="0.35"
                      stroke={SERIES_COLORS[seriesIndex() % SERIES_COLORS.length]}
                      stroke-width="2"
                      points={`20,190 ${series.values.map((value: any, index: any) => `${20 + index * (340 / Math.max(1, chartData().labels.length - 1))},${190 - Math.abs(value) / chartMax() * 170}`).join(" ")} 365,190`}
                    />
                  )}
                </For>
              </Show>
              <Show when={chartType() === "line"}>
                <For each={chartData().series}>
                  {(series: any, seriesIndex: () => number) => (
                    <polyline
                      fill="none"
                      stroke={SERIES_COLORS[seriesIndex() % SERIES_COLORS.length]}
                      stroke-width="3"
                      points={series.values.map((value: any, index: any) => `${20 + index * (340 / Math.max(1, chartData().labels.length - 1))},${190 - Math.abs(value) / chartMax() * 170}`).join(" ")}
                    />
                  )}
                </For>
              </Show>
              <Show when={chartType() === "scatter"}>
                <For each={chartData().series}>
                  {(series: any, seriesIndex: () => number) => (
                    <For each={series.values}>
                      {(value: any, index: () => number) => (
                        <circle
                          cx={20 + index() * (340 / Math.max(1, chartData().labels.length - 1))}
                          cy={190 - Math.abs(value) / chartMax() * 170}
                          r="4"
                          fill={SERIES_COLORS[seriesIndex() % SERIES_COLORS.length]}
                        />
                      )}
                    </For>
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
              <Show when={props.chartTitle}>
                <text x="190" y="14" text-anchor="middle" font-size="12" font-weight="600" fill="#0f172a">{props.chartTitle}</text>
              </Show>
              <Show when={chartType() !== "pie" && chartType() !== "doughnut"}>
                {/* Axis max label for the value scale. */}
                <text x="12" y="26" font-size="9" fill="#64748b">{Math.round(chartMax())}</text>
                <text x="12" y="192" font-size="9" fill="#64748b">0</text>
                <Show when={chartData().labels.length > 0}>
                  <text x="20" y="202" font-size="9" fill="#64748b" text-anchor="start">
                    {chartData().labels[0].length > 12 ? `${chartData().labels[0].slice(0, 11)}…` : chartData().labels[0]}
                  </text>
                  <text x="360" y="202" font-size="9" fill="#64748b" text-anchor="end">
                    {(() => {
                      const last = chartData().labels[chartData().labels.length - 1];
                      return last.length > 12 ? `${last.slice(0, 11)}…` : last;
                    })()}
                  </text>
                </Show>
                {/* Legend: one entry per series, top-right. */}
                <Show when={chartData().series.length > 1}>
                  <For each={chartData().series.slice(0, 6)}>
                    {(series: any, index: () => number) => (
                      <g>
                        <rect x={300} y={30 + index() * 13} width="8" height="8" fill={SERIES_COLORS[index() % SERIES_COLORS.length]} />
                        <text x="312" y={37 + index() * 13} font-size="9" fill="#334155">
                          {series.name.length > 12 ? `${series.name.slice(0, 11)}…` : series.name}
                        </text>
                      </g>
                    )}
                  </For>
                </Show>
               </Show>
             </svg>
            {/* Visually hidden data table for screen readers — WCAG 1.1.1 compliance */}
            <div
              id={`chart-desc-${chartType()}`}
              role="region"
              aria-label={`Data table for ${chartType()} chart`}
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
              <table aria-label={`${props.chartTitle || "Chart"} data`}>
                <caption>{props.chartTitle || "Chart Data"}</caption>
                <thead>
                  <tr>
                    <th scope="col">Label</th>
                    <For each={chartData().series}>
                      {(series: any) => <th scope="col">{series.name || "Series"}</th>}
                    </For>
                  </tr>
                </thead>
                <tbody>
                  <For each={chartData().labels}>
                    {(label: string, labelIndex: () => number) => (
                      <tr>
                        <th scope="row">{label}</th>
                        <For each={chartData().series}>
                          {(series: any) => (
                            <td>{series.values[labelIndex()] ?? ""}</td>
                          )}
                        </For>
                      </tr>
                    )}
                  </For>
                </tbody>
              </table>
            </div>
          </Show>
         </div>

     </>
   );
}
