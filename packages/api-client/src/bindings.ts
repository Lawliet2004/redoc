import { commands as generatedCommands } from "./generated";
import type {
  AppSettings as GeneratedAppSettings,
  DocWordCount,
  OpenedDocument as GeneratedOpenedDocument,
  PptxImportResponse,
  RecentEntry as GeneratedRecentEntry,
  RecoveredDoc,
  RedocMeta as GeneratedRedocMeta,
  SearchMatch,
  Result,
  WorkbookModel,
} from "./generated";

export type {
  CellRange,
  DocWordCount,
  DocxImportResponse,
  PptxImportResponse,
  XlsxImportResponse,
  RecoveredDoc,
  SearchMatch,
  WorkbookModel,
  SheetData,
  SheetCell,
  PivotTableModel,
} from "./generated";

export type AppSettings = Omit<GeneratedAppSettings, "theme"> & {
  theme: "light" | "dark" | "system";
};
export type RecentEntry = Omit<GeneratedRecentEntry, "mode"> & {
  mode: "doc" | "sheet" | "slide";
};
export type RedocMeta = Omit<GeneratedRedocMeta, "mode"> & {
  mode: "doc" | "sheet" | "slide";
};
export type OpenedDocument = Omit<GeneratedOpenedDocument, "meta"> & {
  meta: RedocMeta;
};

function unwrap<T>(result: Result<T, string>): T {
  if (result.status === "ok") return result.data;
  throw new Error(result.error);
}

export const commands = {
  takePendingOpenPaths: async () => unwrap(await generatedCommands.takePendingOpenPaths()),
  getSettings: async () => unwrap(await generatedCommands.getSettings()) as AppSettings,
  updateSettings: async (newSettings: AppSettings) =>
    unwrap(await generatedCommands.updateSettings(newSettings)),
  recordTelemetryEvent: async (event: string) =>
    unwrap(await generatedCommands.recordTelemetryEvent(event)),
  getRecents: async () => unwrap(await generatedCommands.getRecents()) as RecentEntry[],
  togglePinRecent: async (id: string) => unwrap(await generatedCommands.togglePinRecent(id)),
  checkRecovery: async () => unwrap(await generatedCommands.checkRecovery()),
  createNewDocument: async (mode: string, title: string) =>
    unwrap(await generatedCommands.createNewDocument(mode, title)) as OpenedDocument,
  openDocument: async (path: string) =>
    unwrap(await generatedCommands.openDocument(path)) as OpenedDocument,
  openRecoveredDocument: async (path: string) =>
    unwrap(await generatedCommands.openRecoveredDocument(path)) as OpenedDocument,
  discardRecoverySnapshots: async () => unwrap(await generatedCommands.discardRecoverySnapshots()),
  markCleanShutdown: async () => unwrap(await generatedCommands.markCleanShutdown()),
  saveDocument: async (
    path: string,
    mode: string,
    title: string,
    body: unknown,
    documentId?: string | null,
  ) =>
    unwrap(
      await generatedCommands.saveDocument(
        path,
        mode,
        title,
        JSON.stringify(body),
        documentId ?? null,
      ),
    ) as RedocMeta,
  autosaveDocument: async (docId: string, mode: string, title: string, body: unknown) =>
    unwrap(await generatedCommands.autosaveDocument(docId, mode, title, JSON.stringify(body))),
  exportDocument: async (mode: string, format: string, body: unknown, title: string) =>
    unwrap(await generatedCommands.exportDocument(mode, format, JSON.stringify(body), title)),
  inspectExportCompatibility: async (mode: string, format: string, body: unknown) =>
    unwrap(
      await generatedCommands.inspectExportCompatibility(
        mode,
        format,
        JSON.stringify(body),
      ),
    ),
  exportDocumentToFile: async (
    path: string,
    mode: string,
    format: string,
    body: unknown,
    title: string,
  ) =>
    unwrap(
      await generatedCommands.exportDocumentToFile(
        path,
        mode,
        format,
        JSON.stringify(body),
        title,
      ),
    ),
  computeDocWordCount: async (docJson: unknown) =>
    unwrap(await generatedCommands.computeDocWordCount(JSON.stringify(docJson))),
  searchDocText: async (docJson: unknown, query: string, caseSensitive: boolean) =>
    unwrap(await generatedCommands.searchDocText(JSON.stringify(docJson), query, caseSensitive)),
  exportSheetCsv: async (sheetData: unknown) =>
    unwrap(await generatedCommands.exportSheetCsvCmd(sheetData as never)),
  exportCsvToFile: async (path: string, sheetData: unknown) =>
    unwrap(await generatedCommands.exportCsvToFile(path, sheetData as never)),
  importCsvFile: async (path: string) => unwrap(await generatedCommands.importCsvFile(path)),
  importCsvFileWithOptions: async (
    path: string,
    delimiter: string | null,
    encoding: "auto" | "utf-8" | "latin-1",
  ) => unwrap(await generatedCommands.importCsvFileWithOptions(path, delimiter, encoding)),
  importXlsxFile: async (path: string) => unwrap(await generatedCommands.importXlsxFile(path)),
  importDocxFile: async (path: string) => unwrap(await generatedCommands.importDocxFile(path)),
  importPptxFile: async (path: string) => unwrap(await generatedCommands.importPptxFile(path)) as PptxImportResponse,
  recalculateWorkbook: async (workbook: unknown) =>
    unwrap(await generatedCommands.recalculateWorkbook(workbook as WorkbookModel)),
  setWorkbookCellValue: async (
    workbook: unknown,
    sheetIdx: number,
    row: number,
    col: number,
    rawValue: string,
  ) =>
    unwrap(
      await generatedCommands.setWorkbookCellValue(
        workbook as WorkbookModel,
        sheetIdx,
        row,
        col,
        rawValue,
      ),
    ),
  fillSeries: async (
    workbook: unknown,
    sheetIdx: number,
    sourceRow: number,
    sourceCol: number,
    targetRow: number,
    targetCol: number,
  ) =>
    unwrap(
      await generatedCommands.fillWorkbookSeries(
        workbook as WorkbookModel,
        sheetIdx,
        sourceRow,
        sourceCol,
        targetRow,
        targetCol,
      ),
    ),
  sortRange: async (
    workbook: unknown,
    sheetIdx: number,
    startRow: number,
    endRow: number,
    startCol: number,
    endCol: number,
    sortCol: number,
    ascending: boolean,
  ) =>
    unwrap(
      await generatedCommands.sortWorkbookRange(
        workbook as WorkbookModel,
        sheetIdx,
        { startRow, endRow, startCol, endCol },
        sortCol,
        ascending,
      ),
    ),
  sortWorkbookRangeMulti: async (
    workbook: unknown,
    sheetIdx: number,
    startRow: number,
    endRow: number,
    startCol: number,
    endCol: number,
    keys: [number, boolean][],
  ) =>
    unwrap(
      await generatedCommands.sortWorkbookRangeMulti(
        workbook as WorkbookModel,
        sheetIdx,
        { startRow, endRow, startCol, endCol },
        keys,
      ),
    ),
  sortRangeMulti: async (
    workbook: unknown,
    sheetIdx: number,
    startRow: number,
    endRow: number,
    startCol: number,
    endCol: number,
    keys: [number, boolean][],
  ) =>
    unwrap(
      await generatedCommands.sortWorkbookRangeMulti(
        workbook as WorkbookModel,
        sheetIdx,
        { startRow, endRow, startCol, endCol },
        keys,
      ),
    ),
  insertRowsCmd: async (workbook: unknown, sheetIdx: number, atRow: number, count: number) =>
    unwrap(
      await generatedCommands.insertRowsCmd(workbook as WorkbookModel, sheetIdx, atRow, count),
    ),
  deleteRowsCmd: async (workbook: unknown, sheetIdx: number, atRow: number, count: number) =>
    unwrap(
      await generatedCommands.deleteRowsCmd(workbook as WorkbookModel, sheetIdx, atRow, count),
    ),
  insertColsCmd: async (workbook: unknown, sheetIdx: number, atCol: number, count: number) =>
    unwrap(
      await generatedCommands.insertColsCmd(workbook as WorkbookModel, sheetIdx, atCol, count),
    ),
  deleteColsCmd: async (workbook: unknown, sheetIdx: number, atCol: number, count: number) =>
    unwrap(
      await generatedCommands.deleteColsCmd(workbook as WorkbookModel, sheetIdx, atCol, count),
    ),
  setFreezeCmd: async (
    workbook: unknown,
    sheetIdx: number,
    freezeRows: number,
    freezeCols: number,
  ) =>
    unwrap(
      await generatedCommands.setFreezeCmd(
        workbook as WorkbookModel,
        sheetIdx,
        freezeRows,
        freezeCols,
      ),
    ),
  openPresenterWindow: async () => unwrap(await generatedCommands.openPresenterWindow()),
  presenterNav: async (slideIndex: number) => unwrap(await generatedCommands.presenterNav(slideIndex)),
  openLogsFolder: async () => unwrap(await generatedCommands.openLogsFolder()),
  logFrontendError: async (level: string, message: string, stack: string | null) =>
    unwrap(await generatedCommands.logFrontendError(level, message, stack)),
  pathsExist: async (paths: string[]) => await generatedCommands.pathsExist(paths),
  presenterSync: async (payload: { slide_index: number; deck_version: number; elapsed_ms: number; is_playing: boolean }) =>
    unwrap(await generatedCommands.presenterSync(payload)),
};
