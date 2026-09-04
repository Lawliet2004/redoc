use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub enum FormulaValue {
    Number(f64),
    String(String),
    Boolean(bool),
    Error(FormulaError),
    Empty,
    Array(Vec<FormulaValue>, u32, u32),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum FormulaError {
    DivZero, // #DIV/0!
    Ref,     // #REF!
    Name,    // #NAME?
    Value,   // #VALUE!
    Cycle,   // #CYCLE!
    Na,      // #N/A
    Spill,   // #SPILL!
    Num,     // #NUM!
}

impl FormulaError {
    pub fn to_str(&self) -> &'static str {
        match self {
            FormulaError::DivZero => "#DIV/0!",
            FormulaError::Ref => "#REF!",
            FormulaError::Name => "#NAME?",
            FormulaError::Value => "#VALUE!",
            FormulaError::Cycle => "#CYCLE!",
            FormulaError::Na => "#N/A",
            FormulaError::Spill => "#SPILL!",
            FormulaError::Num => "#NUM!",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Neq,
    Lt,
    Lte,
    Gt,
    Gte,
    Concat,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    Literal(FormulaValue),
    CellRef {
        sheet: Option<String>,
        row: u32,
        col: u32,
    },
    RangeRef {
        sheet: Option<String>,
        start_row: u32,
        start_col: u32,
        end_row: u32,
        end_col: u32,
    },
    Name(String),
    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
    },
    FunctionCall {
        name: String,
        args: Vec<Expr>,
    },
}
