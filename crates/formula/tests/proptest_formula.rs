use proptest::prelude::*;
use redoc_formula::{eval_expr, parse_formula, CellProvider, FormulaValue};

struct DummyProvider;
impl CellProvider for DummyProvider {
    fn get_cell_value(&self, _sheet: Option<&str>, _row: u32, _col: u32) -> FormulaValue {
        FormulaValue::Number(0.0)
    }
}

proptest! {
    #[test]
    fn test_random_cell_refs_parse(
        col_letters in "[a-zA-Z]{1,3}",
        row_num in 1..1000000u32,
    ) {
        let formula = format!("={}!{}{}", "Sheet1", col_letters, row_num);
        let _ = parse_formula(&formula); // Should not panic

        let formula2 = format!("={}{}", col_letters, row_num);
        let _ = parse_formula(&formula2); // Should not panic
    }

    #[test]
    fn test_random_integer_arithmetic(
        a in -10000..10000i64,
        b in -10000..10000i64,
    ) {
        let formula_add = format!("={}+{}", a, b);
        if let Ok(ast) = parse_formula(&formula_add) {
            let res = eval_expr(&ast, &DummyProvider);
            assert_eq!(res, FormulaValue::Number((a + b) as f64));
        }

        let formula_sub = format!("={}-{}", a, b);
        if let Ok(ast) = parse_formula(&formula_sub) {
            let res = eval_expr(&ast, &DummyProvider);
            assert_eq!(res, FormulaValue::Number((a - b) as f64));
        }

        let formula_mul = format!("={}*{}", a, b);
        if let Ok(ast) = parse_formula(&formula_mul) {
            let res = eval_expr(&ast, &DummyProvider);
            assert_eq!(res, FormulaValue::Number((a * b) as f64));
        }
    }

    #[test]
    fn test_dollar_prefix_parsing(
        col_letters in "[A-Z]{1,3}",
        row_num in 1..1000000u32,
    ) {
        let formula = format!("={}{}", col_letters, row_num);
        let parsed_normal = parse_formula(&formula);

        let formula_dollar_col = format!("=${}{}", col_letters, row_num);
        let parsed_dollar_col = parse_formula(&formula_dollar_col);

        let formula_dollar_row = format!("={}${}", col_letters, row_num);
        let parsed_dollar_row = parse_formula(&formula_dollar_row);

        let formula_dollar_both = format!("=${}${}", col_letters, row_num);
        let parsed_dollar_both = parse_formula(&formula_dollar_both);

        if parsed_normal.is_ok() {
            assert!(parsed_dollar_col.is_ok());
            assert!(parsed_dollar_row.is_ok());
            assert!(parsed_dollar_both.is_ok());
        }
    }
}
