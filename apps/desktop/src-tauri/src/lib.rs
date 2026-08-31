#![allow(clippy::redundant_closure_call)]

use redoc_core::{AppSettings, AppState, RecentEntry, RecoveredDoc};
use redoc_doc_engine::{DocWordCount, SearchMatch};
use redoc_file_io::{RedocContainer, RedocMeta};
use redoc_sheet_engine::{SheetData, WorkbookModel};
use redoc_slide_engine::DeckModel;
use serde_json::json;
use std::path::Path;
#[cfg(any(windows, target_os = "linux"))]
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};
use tauri_specta::{collect_commands, Builder as SpectaBuilder};

macro_rules! handle_panic {
    ($body:block) => {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| $body)).unwrap_or_else(|err| {
            let msg = if let Some(s) = err.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = err.downcast_ref::<String>() {
                s.clone()
            } else {
                "Unknown panic".to_string()
            };
            Err(format!("Command panicked: {}", msg))
        })
    };
}

struct PendingOpenPaths(Mutex<Vec<String>>);

fn is_redoc_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("redoc"))
        .unwrap_or(false)
}

fn url_decode(input: &str) -> String {
    let mut bytes = Vec::new();
    let input_bytes = input.as_bytes();
    let mut i = 0;
    while i < input_bytes.len() {
        if input_bytes[i] == b'%' && i + 2 < input_bytes.len() {
            if let Ok(b) = u8::from_str_radix(&input[i + 1..i + 3], 16) {
                bytes.push(b);
                i += 3;
                continue;
            }
        }
        bytes.push(input_bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

fn normalize_file_path(arg: &str) -> PathBuf {
    let decoded = url_decode(arg);
    let s = decoded.trim();
    if let Some(rest) = s.strip_prefix("file://") {
        let rest = rest.trim_start_matches('/');
        if cfg!(windows) {
            let rest = rest.strip_prefix("localhost/").unwrap_or(rest);
            let rest_fixed = if rest.len() >= 2 && rest.as_bytes().get(1) == Some(&b'|') {
                format!("{}:{}", &rest[0..1], &rest[2..])
            } else {
                rest.to_string()
            };
            PathBuf::from(rest_fixed.replace('/', "\\"))
        } else {
            PathBuf::from(format!("/{rest}"))
        }
    } else {
        PathBuf::from(s)
    }
}

#[cfg(any(windows, target_os = "linux"))]
fn path_from_arg(arg: &str) -> Option<PathBuf> {
    if arg.starts_with('-') {
        return None;
    }
    Some(normalize_file_path(arg))
}

#[cfg(any(windows, target_os = "linux"))]
fn collect_argv_redoc_paths() -> Vec<String> {
    std::env::args()
        .skip(1)
        .filter_map(|arg| path_from_arg(&arg))
        .filter(|path| is_redoc_path(path))
        .map(|path| path.to_string_lossy().into_owned())
        .collect()
}

fn queue_open_paths(app: &tauri::AppHandle, paths: Vec<String>) {
    if paths.is_empty() {
        return;
    }
    if let Some(pending) = app.try_state::<PendingOpenPaths>() {
        pending
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .extend(paths.clone());
    }
    let _ = app.emit("redoc-open-file", paths);
}

#[tauri::command]
#[specta::specta]
fn take_pending_open_paths(
    state: tauri::State<'_, PendingOpenPaths>,
) -> Result<Vec<String>, String> {
    handle_panic!({
        let res = (|| std::mem::take(&mut *state.0.lock().unwrap_or_else(|e| e.into_inner())))();
        Ok(res)
    })
}

#[tauri::command]
#[specta::specta]
fn get_settings(state: tauri::State<'_, Arc<AppState>>) -> Result<AppSettings, String> {
    handle_panic!({
        let res = (|| state.settings.read().clone())();
        Ok(res)
    })
}

#[tauri::command]
#[specta::specta]
fn update_settings(
    state: tauri::State<'_, Arc<AppState>>,
    new_settings: AppSettings,
) -> Result<(), String> {
    handle_panic!({
        *state.settings.write() = new_settings;
        let _ = state.persist_settings();
        Ok(())
    })
}

#[tauri::command]
#[specta::specta]
fn record_telemetry_event(
    state: tauri::State<'_, Arc<AppState>>,
    event: String,
) -> Result<(), String> {
    handle_panic!({
        #[cfg(feature = "telemetry")]
        if state.settings.read().telemetry_enabled {
            tracing::info!(target: "redoc::telemetry", event = %event, "local telemetry event");
        }
        #[cfg(not(feature = "telemetry"))]
        let _ = (state, event);
        Ok(())
    })
}

#[tauri::command]
#[specta::specta]
fn get_recents(state: tauri::State<'_, Arc<AppState>>) -> Result<Vec<RecentEntry>, String> {
    handle_panic!({
        let res = (|| state.recents.read().entries.clone())();
        Ok(res)
    })
}

#[tauri::command]
#[specta::specta]
fn toggle_pin_recent(state: tauri::State<'_, Arc<AppState>>, id: String) -> Result<bool, String> {
    handle_panic!({
        let res = (|| {
            let pinned = state.recents.write().toggle_pin(&id);
            let _ = state.persist_recents();
            pinned
        })();
        Ok(res)
    })
}

#[tauri::command]
#[specta::specta]
fn check_recovery(state: tauri::State<'_, Arc<AppState>>) -> Result<Vec<RecoveredDoc>, String> {
    handle_panic!({
        let res = (|| {
            if state.recovery_pending {
                state.recovery.check_recovery_needed()
            } else {
                Vec::new()
            }
        })();
        Ok(res)
    })
}

#[derive(serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
struct OpenedDocument {
    meta: RedocMeta,
    body: UntypedJson,
}

#[derive(serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
struct DocxImportResponse {
    document: UntypedJson,
    warnings: Vec<String>,
}

#[derive(serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
struct XlsxImportResponse {
    workbook: WorkbookModel,
    warnings: Vec<String>,
}

#[derive(serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
struct PptxImportResponse {
    deck: DeckModel,
    warnings: Vec<String>,
}

#[derive(serde::Serialize)]
#[serde(transparent)]
struct UntypedJson(serde_json::Value);

impl specta::Type for UntypedJson {
    fn inline(_: &mut specta::TypeMap, _: specta::Generics) -> specta::DataType {
        specta::DataType::Any
    }
}

#[tauri::command]
#[specta::specta]
fn create_new_document(
    _state: tauri::State<'_, Arc<AppState>>,
    mode: String,
    title: String,
) -> Result<OpenedDocument, String> {
    handle_panic!({
        let initial_body = match mode.as_str() {
            "doc" => {
                json!({ "type": "doc", "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "Welcome to Redoc Document Editor!" }] }] })
            }
            "sheet" => {
                serde_json::to_value(WorkbookModel::new_default()).map_err(|e| e.to_string())?
            }
            "slide" => serde_json::to_value(DeckModel::new_default()).map_err(|e| e.to_string())?,
            _ => return Err("Invalid mode".to_string()),
        };

        let container = RedocContainer::new(&mode, &title, initial_body);
        Ok(OpenedDocument {
            meta: container.meta,
            body: UntypedJson(container.body),
        })
    })
}

#[tauri::command]
#[specta::specta]
fn open_document(
    state: tauri::State<'_, Arc<AppState>>,
    path: String,
) -> Result<OpenedDocument, String> {
    handle_panic!({
        let normalized = normalize_file_path(&path);
        let container = RedocContainer::read_from_file(&normalized).map_err(|e| e.to_string())?;
        let path_str = normalized.to_string_lossy().to_string();
        state.recents.write().add(
            path_str,
            container.meta.title.clone(),
            container.meta.mode.clone(),
        );
        let _ = state.persist_recents();
        Ok(OpenedDocument {
            meta: container.meta,
            body: UntypedJson(container.body),
        })
    })
}

#[tauri::command]
#[specta::specta]
fn open_recovered_document(path: String) -> Result<OpenedDocument, String> {
    handle_panic!({
        let normalized = normalize_file_path(&path);
        let container = RedocContainer::read_from_file(&normalized).map_err(|e| e.to_string())?;
        Ok(OpenedDocument {
            meta: container.meta,
            body: UntypedJson(container.body),
        })
    })
}

#[tauri::command]
#[specta::specta]
fn discard_recovery_snapshots(state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
    handle_panic!({
        state
            .recovery
            .clear_all_snapshots()
            .map_err(|e| e.to_string())?;
        Ok(())
    })
}

#[tauri::command]
#[specta::specta]
fn mark_clean_shutdown(state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
    handle_panic!({
        state
            .recovery
            .mark_app_stopped()
            .map_err(|e| e.to_string())?;
        Ok(())
    })
}

#[tauri::command]
#[specta::specta]
fn save_document(
    state: tauri::State<'_, Arc<AppState>>,
    path: String,
    mode: String,
    title: String,
    body_json: String,
    document_id: Option<String>,
) -> Result<RedocMeta, String> {
    handle_panic!({
        let normalized = normalize_file_path(&path);
        let body: serde_json::Value =
            serde_json::from_str(&body_json).map_err(|e| e.to_string())?;
        let mut container = RedocContainer::new(&mode, &title, body);
        container.add_inline_data_uri_assets();
        if let Some(id) = document_id {
            container.meta.id = id;
        }
        container
            .save_atomic(&normalized)
            .map_err(|e| e.to_string())?;
        let _ = redoc_file_io::SnapshotManager::new(state.recovery.autosave_dir())
            .remove_snapshot(&container.meta.id);
        let path_str = normalized.to_string_lossy().to_string();
        state.recents.write().add(path_str, title, mode);
        let _ = state.persist_recents();
        Ok(container.meta)
    })
}

#[tauri::command]
#[specta::specta]
fn autosave_document(
    state: tauri::State<'_, Arc<AppState>>,
    doc_id: String,
    mode: String,
    title: String,
    body_json: String,
) -> Result<String, String> {
    handle_panic!({
        let body: serde_json::Value =
            serde_json::from_str(&body_json).map_err(|e| e.to_string())?;
        let mut container = RedocContainer::new(&mode, &title, body);
        container.meta.id = doc_id.clone();
        container.add_inline_data_uri_assets();
        let snapshots = redoc_file_io::SnapshotManager::new(state.recovery.autosave_dir());
        let path = snapshots
            .write_snapshot(&doc_id, &mut container)
            .map_err(|e| e.to_string())?;
        Ok(path.to_string_lossy().to_string())
    })
}

#[tauri::command]
#[specta::specta]
fn export_document(
    mode: String,
    format: String,
    body_json: String,
    title: String,
) -> Result<Vec<u8>, String> {
    handle_panic!({
        let body: serde_json::Value =
            serde_json::from_str(&body_json).map_err(|e| e.to_string())?;
        match format.as_str() {
            "pdf" => match mode.as_str() {
                "sheet" => {
                    let workbook: WorkbookModel =
                        serde_json::from_value(body).map_err(|e| e.to_string())?;
                    redoc_export::export_workbook_to_pdf(&workbook, &title)
                        .map_err(|e| e.to_string())
                }
                "slide" => {
                    let deck: DeckModel =
                        serde_json::from_value(body).map_err(|e| e.to_string())?;
                    redoc_export::export_deck_to_pdf(&deck, &title).map_err(|e| e.to_string())
                }
                _ => redoc_export::export_doc_to_pdf(&body, &title).map_err(|e| e.to_string()),
            },
            "docx" => redoc_export::export_doc_to_docx(&body, &title).map_err(|e| e.to_string()),
            "xlsx" => {
                let wb: WorkbookModel = serde_json::from_value(body).map_err(|e| e.to_string())?;
                redoc_export::export_workbook_to_xlsx(&wb).map_err(|e| e.to_string())
            }
            "pptx" => {
                let deck: DeckModel = serde_json::from_value(body).map_err(|e| e.to_string())?;
                redoc_export::export_deck_to_pptx(&deck).map_err(|e| e.to_string())
            }
            _ => Err("Unsupported export format".to_string()),
        }
    })
}

#[tauri::command]
#[specta::specta]
fn inspect_export_compatibility(
    mode: String,
    format: String,
    body_json: String,
) -> Result<Vec<String>, String> {
    handle_panic!({
        let body: serde_json::Value =
            serde_json::from_str(&body_json).map_err(|e| e.to_string())?;
        Ok(redoc_export::export_compatibility_warnings(
            &mode, &format, &body,
        ))
    })
}

#[tauri::command]
#[specta::specta]
fn export_document_to_file(
    path: String,
    mode: String,
    format: String,
    body_json: String,
    title: String,
) -> Result<(), String> {
    handle_panic!({
        let normalized = normalize_file_path(&path);
        let bytes = export_document(mode, format, body_json, title)?;
        redoc_file_io::write_bytes_atomic(&normalized, &bytes).map_err(|e| e.to_string())
    })
}

#[tauri::command]
#[specta::specta]
fn compute_doc_word_count(doc_json_str: String) -> Result<DocWordCount, String> {
    handle_panic!({
        let res = (|| {
            let doc_json: serde_json::Value =
                serde_json::from_str(&doc_json_str).unwrap_or(json!({}));
            redoc_doc_engine::compute_word_count(&doc_json)
        })();
        Ok(res)
    })
}

#[tauri::command]
#[specta::specta]
fn search_doc_text(
    doc_json_str: String,
    query: String,
    case_sensitive: bool,
) -> Result<Vec<SearchMatch>, String> {
    handle_panic!({
        let res = (|| {
            let doc_json: serde_json::Value =
                serde_json::from_str(&doc_json_str).unwrap_or(json!({}));
            redoc_doc_engine::search_doc(&doc_json, &query, case_sensitive)
        })();
        Ok(res)
    })
}

#[tauri::command]
#[specta::specta]
fn export_sheet_csv_cmd(sheet_data: SheetData) -> Result<String, String> {
    handle_panic!({
        let res = (|| redoc_sheet_engine::export_sheet_to_csv(&sheet_data))();
        Ok(res)
    })
}

#[tauri::command]
#[specta::specta]
fn export_csv_to_file(path: String, sheet_data: SheetData) -> Result<(), String> {
    handle_panic!({
        let normalized = normalize_file_path(&path);
        std::fs::write(
            normalized,
            redoc_sheet_engine::export_sheet_to_csv(&sheet_data),
        )
        .map_err(|e| e.to_string())
    })
}

#[tauri::command]
#[specta::specta]
fn import_csv_file(path: String) -> Result<WorkbookModel, String> {
    handle_panic!({
        let normalized = normalize_file_path(&path);
        let csv = std::fs::read(&normalized).map_err(|e| e.to_string())?;
        let name = normalized
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("Imported CSV");
        let sheet = redoc_sheet_engine::import_csv_bytes_to_sheet(&csv, name);
        let mut workbook = redoc_sheet_engine::WorkbookModel::new_default();
        workbook.sheets = vec![sheet];
        Ok(workbook)
    })
}

#[tauri::command]
#[specta::specta]
fn import_csv_file_with_options(
    path: String,
    delimiter: Option<String>,
    encoding: String,
) -> Result<WorkbookModel, String> {
    handle_panic!({
        let normalized = normalize_file_path(&path);
        let csv = std::fs::read(&normalized).map_err(|e| e.to_string())?;
        let name = normalized
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("Imported CSV");
        let delimiter = delimiter.and_then(|value| value.chars().next());
        let sheet = redoc_sheet_engine::import_csv_bytes_to_sheet_with_options(
            &csv, name, delimiter, &encoding,
        );
        let mut workbook = redoc_sheet_engine::WorkbookModel::new_default();
        workbook.sheets = vec![sheet];
        Ok(workbook)
    })
}

#[tauri::command]
#[specta::specta]
fn import_xlsx_file(path: String) -> Result<XlsxImportResponse, String> {
    handle_panic!({
        let normalized = normalize_file_path(&path);
        redoc_export::import_workbook_from_xlsx_with_report(&normalized)
            .map(|result| XlsxImportResponse {
                workbook: result.workbook,
                warnings: result.warnings,
            })
            .map_err(|e| e.to_string())
    })
}

#[tauri::command]
#[specta::specta]
fn import_docx_file(path: String) -> Result<DocxImportResponse, String> {
    handle_panic!({
        let normalized = normalize_file_path(&path);
        redoc_export::import_docx_to_doc_with_report(&normalized)
            .map(|result| DocxImportResponse {
                document: UntypedJson(result.document),
                warnings: result.warnings,
            })
            .map_err(|e| e.to_string())
    })
}

#[tauri::command]
#[specta::specta]
fn import_pptx_file(path: String) -> Result<PptxImportResponse, String> {
    handle_panic!({
        let normalized = normalize_file_path(&path);
        redoc_export::import_deck_from_pptx_with_report(&normalized)
            .map(|result| PptxImportResponse {
                deck: result.deck,
                warnings: result.warnings,
            })
            .map_err(|e| e.to_string())
    })
}

#[tauri::command]
#[specta::specta]
fn recalculate_workbook(mut workbook: WorkbookModel) -> Result<WorkbookModel, String> {
    handle_panic!({
        let res = (|| {
            for _ in 0..=workbook.sheets.len() {
                for index in 0..workbook.sheets.len() {
                    workbook.recalculate(index);
                }
            }
            workbook
        })();
        Ok(res)
    })
}

#[tauri::command]
#[specta::specta]
fn set_workbook_cell_value(
    mut workbook: WorkbookModel,
    sheet_idx: usize,
    row: u32,
    col: u32,
    raw_value: String,
) -> Result<WorkbookModel, String> {
    handle_panic!({
        let res = (|| {
            workbook.set_cell_value(sheet_idx, row, col, raw_value);
            workbook
        })();
        Ok(res)
    })
}

#[tauri::command]
#[specta::specta]
fn fill_workbook_series(
    mut workbook: WorkbookModel,
    sheet_idx: usize,
    source_row: u32,
    source_col: u32,
    target_row: u32,
    target_col: u32,
) -> Result<WorkbookModel, String> {
    handle_panic!({
        let res = (|| {
            workbook.fill_series(sheet_idx, source_row, source_col, target_row, target_col);
            workbook
        })();
        Ok(res)
    })
}

#[tauri::command]
#[specta::specta]
fn sort_workbook_range(
    mut workbook: WorkbookModel,
    sheet_idx: usize,
    range: redoc_sheet_engine::CellRange,
    sort_col: u32,
    ascending: bool,
) -> Result<WorkbookModel, String> {
    handle_panic!({
        let res = (|| {
            workbook.sort_range(sheet_idx, range, sort_col, ascending);
            workbook
        })();
        Ok(res)
    })
}

#[tauri::command]
#[specta::specta]
fn sort_workbook_range_multi(
    mut workbook: WorkbookModel,
    sheet_idx: usize,
    range: redoc_sheet_engine::CellRange,
    keys: Vec<(u32, bool)>,
) -> Result<WorkbookModel, String> {
    handle_panic!({
        let res = (|| {
            workbook.sort_range_multi(sheet_idx, range, &keys);
            workbook
        })();
        Ok(res)
    })
}

#[tauri::command]
#[specta::specta]
fn open_presenter_window(app: tauri::AppHandle) -> Result<(), String> {
    handle_panic!({
        let window_label = "presenter";
        if let Some(window) = app.get_webview_window(window_label) {
            let _ = window.show();
            let _ = window.set_focus();
            return Ok(());
        }
        let window = tauri::WebviewWindowBuilder::new(
            &app,
            window_label,
            tauri::WebviewUrl::App("index.html?presenter=true".into()),
        )
        .title("Redoc Presenter")
        .inner_size(1280.0, 800.0)
        .build()
        .map_err(|e| e.to_string())?;

        let _ = window.show();
        let _ = window.set_focus();
        Ok(())
    })
}

#[tauri::command]
#[specta::specta]
fn presenter_nav(app: tauri::AppHandle, slide_index: usize) -> Result<(), String> {
    handle_panic!({
        let _ = app.emit("slide_changed", json!({ "slideIndex": slide_index }));
        Ok(())
    })
}

#[tauri::command]
#[specta::specta]
fn open_logs_folder(state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
    handle_panic!({
        let log_dir = state.log_dir();
        std::fs::create_dir_all(&log_dir).map_err(|e| e.to_string())?;

        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("explorer")
                .arg(&log_dir)
                .spawn()
                .map_err(|e| e.to_string())?;
        }
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open")
                .arg(&log_dir)
                .spawn()
                .map_err(|e| e.to_string())?;
        }
        #[cfg(target_os = "linux")]
        {
            std::process::Command::new("xdg-open")
                .arg(&log_dir)
                .spawn()
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    })
}

#[tauri::command]
#[specta::specta]
fn insert_rows_cmd(
    mut workbook: WorkbookModel,
    sheet_idx: usize,
    at_row: u32,
    count: u32,
) -> Result<WorkbookModel, String> {
    handle_panic!({
        let res = (|| {
            workbook.insert_rows(sheet_idx, at_row, count);
            workbook
        })();
        Ok(res)
    })
}

#[tauri::command]
#[specta::specta]
fn delete_rows_cmd(
    mut workbook: WorkbookModel,
    sheet_idx: usize,
    at_row: u32,
    count: u32,
) -> Result<WorkbookModel, String> {
    handle_panic!({
        let res = (|| {
            workbook.delete_rows(sheet_idx, at_row, count);
            workbook
        })();
        Ok(res)
    })
}

#[tauri::command]
#[specta::specta]
fn insert_cols_cmd(
    mut workbook: WorkbookModel,
    sheet_idx: usize,
    at_col: u32,
    count: u32,
) -> Result<WorkbookModel, String> {
    handle_panic!({
        let res = (|| {
            workbook.insert_cols(sheet_idx, at_col, count);
            workbook
        })();
        Ok(res)
    })
}

#[tauri::command]
#[specta::specta]
fn delete_cols_cmd(
    mut workbook: WorkbookModel,
    sheet_idx: usize,
    at_col: u32,
    count: u32,
) -> Result<WorkbookModel, String> {
    handle_panic!({
        let res = (|| {
            workbook.delete_cols(sheet_idx, at_col, count);
            workbook
        })();
        Ok(res)
    })
}

#[tauri::command]
#[specta::specta]
fn set_freeze_cmd(
    mut workbook: WorkbookModel,
    sheet_idx: usize,
    freeze_rows: u32,
    freeze_cols: u32,
) -> Result<WorkbookModel, String> {
    handle_panic!({
        let res = (|| {
            workbook.set_freeze(sheet_idx, freeze_rows, freeze_cols);
            workbook
        })();
        Ok(res)
    })
}

#[tauri::command]
#[specta::specta]
fn log_frontend_error(level: String, message: String, stack: Option<String>) -> Result<(), String> {
    handle_panic!({
        match level.as_str() {
            "error" => tracing::error!(target: "redoc::frontend", stack = ?stack, "{}", message),
            "warn" => tracing::warn!(target: "redoc::frontend", stack = ?stack, "{}", message),
            "info" => tracing::info!(target: "redoc::frontend", stack = ?stack, "{}", message),
            "debug" => tracing::debug!(target: "redoc::frontend", stack = ?stack, "{}", message),
            _ => tracing::error!(target: "redoc::frontend", stack = ?stack, "{}", message),
        }
        Ok(())
    })
}

#[tauri::command]
#[specta::specta]
fn paths_exist(paths: Vec<String>) -> Vec<bool> {
    paths.iter().map(|p| std::path::Path::new(p).is_file()).collect()
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Debug, Clone)]
pub struct PresenterSyncPayload {
    pub slide_index: u32,
    pub deck_version: u64,
    pub elapsed_ms: u64,
    pub is_playing: bool,
}

#[tauri::command]
#[specta::specta]
fn presenter_sync(app: tauri::AppHandle, payload: PresenterSyncPayload) -> Result<(), String> {
    handle_panic!({
        let _ = app.emit("presenter-sync", payload);
        Ok(())
    })
}

fn specta_builder() -> SpectaBuilder<tauri::Wry> {
    SpectaBuilder::<tauri::Wry>::new().commands(collect_commands![
        take_pending_open_paths,
        get_settings,
        update_settings,
        record_telemetry_event,
        get_recents,
        toggle_pin_recent,
        check_recovery,
        create_new_document,
        open_document,
        open_recovered_document,
        discard_recovery_snapshots,
        mark_clean_shutdown,
        save_document,
        autosave_document,
        export_document,
        inspect_export_compatibility,
        export_document_to_file,
        compute_doc_word_count,
        search_doc_text,
        export_sheet_csv_cmd,
        export_csv_to_file,
        import_csv_file,
        import_csv_file_with_options,
        import_xlsx_file,
        import_docx_file,
        import_pptx_file,
        recalculate_workbook,
        set_workbook_cell_value,
        fill_workbook_series,
        sort_workbook_range,
        sort_workbook_range_multi,
        open_presenter_window,
        presenter_nav,
        open_logs_folder,
        insert_rows_cmd,
        delete_rows_cmd,
        insert_cols_cmd,
        delete_cols_cmd,
        set_freeze_cmd,
        log_frontend_error,
        paths_exist,
        presenter_sync,
    ])
}

fn bindings_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../packages/api-client/src/generated.ts")
}

pub fn generate_typescript_bindings() {
    specta_builder()
        .export(
            specta_typescript::Typescript::default()
                .bigint(specta_typescript::BigIntExportBehavior::Number),
            bindings_path(),
        )
        .expect("failed to export TypeScript bindings");
}

pub fn run() {
    #[cfg(debug_assertions)]
    generate_typescript_bindings();

    let specta_builder = specta_builder();

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_window_state::Builder::default().build());
    #[cfg(feature = "updater")]
    let app = app.plugin(tauri_plugin_updater::Builder::new().build());
    app.manage(PendingOpenPaths(Mutex::new(Vec::new())))
        .setup(|app| {
            let app_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."));
            let state = Arc::new(AppState::new(app_dir));
            redoc_core::init_logging(&state.log_dir());
            let _ = state.recovery.mark_app_started();
            app.manage(state);

            #[cfg(any(windows, target_os = "linux"))]
            {
                let paths = collect_argv_redoc_paths();
                queue_open_paths(app.handle(), paths);
            }

            Ok(())
        })
        .invoke_handler(specta_builder.invoke_handler())
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            if let tauri::RunEvent::Opened { urls } = &event {
                let paths = urls
                    .iter()
                    .filter_map(|url| url.to_file_path().ok())
                    .filter(|path| is_redoc_path(path))
                    .map(|path| path.to_string_lossy().into_owned())
                    .collect::<Vec<_>>();
                queue_open_paths(app, paths);
            }
            #[cfg(not(any(target_os = "macos", target_os = "ios")))]
            {
                let _ = (app, event);
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_file_path_windows_uri() {
        let uri = "file:///C:/Users/Test%20User/Document.redoc";
        let path = normalize_file_path(uri);
        if cfg!(windows) {
            assert_eq!(
                path.to_str().unwrap(),
                "C:\\Users\\Test User\\Document.redoc"
            );
        } else {
            assert_eq!(path.to_str().unwrap(), "C:/Users/Test User/Document.redoc");
        }
    }

    #[test]
    fn test_normalize_file_path_windows_pipe() {
        let uri = "file:///C|/Users/Test/Doc.redoc";
        let path = normalize_file_path(uri);
        if cfg!(windows) {
            assert_eq!(path.to_str().unwrap(), "C:\\Users\\Test\\Doc.redoc");
        }
    }

    #[test]
    fn test_normalize_file_path_plain_path() {
        let plain = "C:\\Users\\Test\\Document.redoc";
        let path = normalize_file_path(plain);
        assert_eq!(path, PathBuf::from(plain));
    }
}
