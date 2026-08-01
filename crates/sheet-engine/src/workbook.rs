//! Workbook orchestration: model, operations, formula rewrite, filters, and recalc.

pub mod model;
pub mod formula_rewrite;
pub mod filter;
pub mod ops;
mod workbook_core;

pub use model::{
    CellRange, ChartModel, NamedRange, RecalcPlan, SheetData, WorkbookModel,
};
pub use formula_rewrite::{
    adjust_formula_references, adjust_formula_references_advanced, col_to_letters,
    parse_a1_reference_details, ParsedRef,
};
pub use workbook_core::parse_key;
