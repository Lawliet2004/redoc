//! Workbook orchestration: model, operations, formula rewrite, filters, and recalc.

pub mod filter;
pub mod formula_rewrite;
pub mod model;
pub mod ops;
mod workbook_core;

pub use formula_rewrite::{
    adjust_formula_references, adjust_formula_references_advanced, col_to_letters,
    parse_a1_reference_details, ParsedRef,
};
pub use model::{
    ArraySpill, CellRange, ChartModel, ConditionalFormattingRange, ConditionalFormattingRule,
    ConditionalFormattingStyle, NamedRange, PivotTableModel, RecalcPlan, ScenarioCellChange,
    ScenarioModel, SheetData, SlicerModel, TableModel, WorkbookModel, MAX_ARRAY_SPILL_CELLS,
};
pub use workbook_core::parse_key;
