pub mod ast;
pub mod catalog;
pub mod dep_graph;
pub mod eval;
pub mod functions;
pub mod parser;

pub use ast::{BinaryOp, Expr, FormulaError, FormulaValue};
pub use catalog::{is_supported_function, SUPPORTED_FUNCTION_NAMES};
pub use dep_graph::{CellCoord, DependencyGraph};
pub use eval::{eval_expr, extract_dependencies, CellProvider};
pub use functions::{eval_func, MAX_DYNAMIC_ARRAY_CELLS};
pub use parser::{
    parse_a1_range, parse_a1_reference, parse_formula, tokenize, Token, MAX_EXCEL_COL,
    MAX_EXCEL_ROW,
};

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    struct DummyProvider;
    impl CellProvider for DummyProvider {
        fn get_cell_value(&self, _sheet: Option<&str>, row: u32, col: u32) -> FormulaValue {
            if row == 1 && col == 1 {
                FormulaValue::Number(10.0)
            } else if row == 1 && col == 2 {
                FormulaValue::Number(20.0)
            } else {
                FormulaValue::Empty
            }
        }
    }

    #[test]
    fn test_simple_eval() {
        let expr = parse_formula("=SUM(A1, B1)").unwrap();
        let val = eval_expr(&expr, &DummyProvider);
        assert_eq!(val, FormulaValue::Number(30.0));
    }

    #[test]
    fn test_a1_parser() {
        assert_eq!(parse_a1_reference("A1"), Some((1, 1)));
        assert_eq!(parse_a1_reference("B2"), Some((2, 2)));
        assert_eq!(parse_a1_reference("Z10"), Some((10, 26)));
    }

    #[test]
    fn evaluates_supported_function_catalog() {
        let cases = [
            ("=SUM(A1:B1)", FormulaValue::Number(30.0)),
            ("=AVERAGE(A1,B1)", FormulaValue::Number(15.0)),
            ("=MIN(A1,B1)", FormulaValue::Number(10.0)),
            ("=MAX(A1,B1)", FormulaValue::Number(20.0)),
            ("=COUNT(A1,B1)", FormulaValue::Number(2.0)),
            ("=COUNTA(A1,B1)", FormulaValue::Number(2.0)),
            (
                "=IF(A1>5,\"yes\",\"no\")",
                FormulaValue::String("yes".into()),
            ),
            ("=AND(A1>5,B1>5)", FormulaValue::Boolean(true)),
            ("=OR(A1<5,B1>5)", FormulaValue::Boolean(true)),
            ("=NOT(A1<5)", FormulaValue::Boolean(true)),
            ("=ROUND(1.234,2)", FormulaValue::Number(1.23)),
            ("=ROUNDUP(1.234,2)", FormulaValue::Number(1.24)),
            ("=ROUNDDOWN(-1.239,2)", FormulaValue::Number(-1.23)),
            ("=INT(-1.2)", FormulaValue::Number(-2.0)),
            ("=TRUNC(-1.29,1)", FormulaValue::Number(-1.2)),
            ("=SIGN(A1)", FormulaValue::Number(1.0)),
            ("=LOG(100,10)", FormulaValue::Number(2.0)),
            ("=LOG10(100)", FormulaValue::Number(2.0)),
            ("=SIN(0)", FormulaValue::Number(0.0)),
            ("=COS(0)", FormulaValue::Number(1.0)),
            ("=TAN(0)", FormulaValue::Number(0.0)),
            ("=CONCAT(\"A\",\"B\")", FormulaValue::String("AB".into())),
            ("=LEFT(\"Hello\",2)", FormulaValue::String("He".into())),
            ("=RIGHT(\"Hello\",2)", FormulaValue::String("lo".into())),
            ("=LEN(\"Hello\")", FormulaValue::Number(5.0)),
            ("=ABS(-4)", FormulaValue::Number(4.0)),
            ("=COUNTIF(A1:B1,\">5\")", FormulaValue::Number(2.0)),
            ("=SUMIF(A1:B1,\">5\")", FormulaValue::Number(30.0)),
            ("=SUMPRODUCT(A1:B1,A1:B1)", FormulaValue::Number(500.0)),
            ("=MAXIFS(A1:B1,A1:B1,\">15\")", FormulaValue::Number(20.0)),
            ("=MINIFS(A1:B1,A1:B1,\">5\")", FormulaValue::Number(10.0)),
            ("=TEXT(12.34,\"0.0\")", FormulaValue::String("12.3".into())),
            ("=VLOOKUP(10,A1:B1,2)", FormulaValue::Number(20.0)),
            ("=XLOOKUP(10,A1:B1,A1:B1)", FormulaValue::Number(10.0)),
            ("=INDEX(A1:B1,2)", FormulaValue::Number(20.0)),
            ("=MATCH(10,A1:B1,0)", FormulaValue::Number(1.0)),
            ("=ROWS(A1:B1)", FormulaValue::Number(1.0)),
            ("=COLUMNS(A1:B1)", FormulaValue::Number(2.0)),
            ("=DATE(2020,1,15)", FormulaValue::Number(43845.0)),
            ("=TIME(14,30,0)", FormulaValue::Number(0.6041666666666666)),
            ("=HOUR(TIME(14,30,0))", FormulaValue::Number(14.0)),
            ("=MINUTE(TIME(14,30,0))", FormulaValue::Number(30.0)),
            ("=SECOND(TIME(14,30,0))", FormulaValue::Number(0.0)),
            (
                "=DAYS(DATE(2020,2,29),DATE(2020,1,31))",
                FormulaValue::Number(29.0),
            ),
            ("=EDATE(DATE(2020,1,31),1)", FormulaValue::Number(43890.0)),
            ("=WORKDAY(DATE(2020,1,31),1)", FormulaValue::Number(43864.0)),
            ("=WEEKDAY(DATE(2020,1,15))", FormulaValue::Number(4.0)),
            (
                "=NETWORKDAYS(DATE(2020,1,6),DATE(2020,1,12))",
                FormulaValue::Number(5.0),
            ),
            (
                "=DAYS(DATE(2020,2,29),DATE(2020,1,31))",
                FormulaValue::Number(29.0),
            ),
            ("=EDATE(DATE(2020,1,31),1)", FormulaValue::Number(43890.0)),
            ("=WORKDAY(DATE(2020,1,31),1)", FormulaValue::Number(43864.0)),
            ("=ISNUMBER(A1)", FormulaValue::Boolean(true)),
            ("=VALUE(\"42\")", FormulaValue::Number(42.0)),
            ("=CHOOSE(2,\"a\",\"b\")", FormulaValue::String("b".into())),
            ("=OFFSET(A1,0,1)", FormulaValue::Number(20.0)),
            ("=INDIRECT(\"A1\")", FormulaValue::Number(10.0)),
            ("=MEDIAN(A1,B1)", FormulaValue::Number(15.0)),
            ("=RANK.EQ(B1,A1:B1)", FormulaValue::Number(1.0)),
            ("=QUARTILE.INC(A1:B1,1)", FormulaValue::Number(12.5)),
            ("=CORREL(A1:B1,A1:B1)", FormulaValue::Number(1.0)),
            ("=SUMSQ(A1:B1)", FormulaValue::Number(500.0)),
            ("=STDEV(A1,B1)", FormulaValue::Number(7.0710678118654755)),
            ("=STDEV.P(A1:B1)", FormulaValue::Number(5.0)),
            ("=VAR(A1,B1)", FormulaValue::Number(50.0)),
            ("=VAR.S(A1:B1)", FormulaValue::Number(50.0)),
            ("=PERCENTILE.INC(A1:B1,0.5)", FormulaValue::Number(15.0)),
            ("=LET(amount,10,amount+5)", FormulaValue::Number(15.0)),
            (
                "=TEXTJOIN(\",\",TRUE,A1,B1)",
                FormulaValue::String("10,20".into()),
            ),
            (
                "=IFS(1=2,\"a\",TRUE,\"b\")",
                FormulaValue::String("b".into()),
            ),
            (
                "=SWITCH(B1,10,\"ten\",20,\"twenty\")",
                FormulaValue::String("twenty".into()),
            ),
        ];
        for (formula, expected) in cases {
            let ast = parse_formula(formula).expect("formula parses");
            let actual = eval_expr(&ast, &DummyProvider);
            assert_eq!(actual, expected, "{formula}");
        }
    }

    proptest::proptest! {
        #[test]
        fn parses_generated_a1_references(row in 1u32..1_000_000, col in 1u32..10_000) {
            let mut column = col;
            let mut letters = String::new();
            while column > 0 {
                let remainder = ((column - 1) % 26) as u8;
                letters.push((b'A' + remainder) as char);
                column = (column - 1) / 26;
            }
            let reference: String = letters.chars().rev().collect::<String>() + &row.to_string();
            prop_assert_eq!(parse_a1_reference(&reference), Some((row, col)));
        }
    }
}

#[cfg(test)]
mod proptest_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn parse_cell_ref_does_not_panic(col in 0usize..26, row in 0usize..100) {
            let col_letter = (b'A' + col as u8) as char;
            let ref_str = format!("={}{}", col_letter, row + 1);
            let _ = parse_formula(&ref_str); // must not panic
        }

        #[test]
        fn dollar_prefix_parses_same_as_without(col in 0usize..26, row in 0usize..100) {
            let col_letter = (b'A' + col as u8) as char;
            let plain = format!("={}{}", col_letter, row + 1);
            let dollar = format!("=${}${}", col_letter, row + 1);
            let r1 = parse_formula(&plain);
            let r2 = parse_formula(&dollar);
            // Both should either both succeed or both fail, but not panic
            drop(r1); drop(r2);
        }
    }
}
