/// Formula names accepted by the evaluator.
///
/// Keep this list sorted and include aliases that Excel workbooks commonly use.
/// The TypeScript formula picker is generated from this constant by
/// `scripts/generate-formula-catalog.mjs`.
pub const SUPPORTED_FUNCTION_NAMES: &[&str] = &[
    "ABS",
    "AND",
    "AVERAGE",
    "AVERAGEIF",
    "AVERAGEIFS",
    "CEILING",
    "CHOOSE",
    "CONCAT",
    "COUNT",
    "COUNTA",
    "COUNTIF",
    "COUNTIFS",
    "DATE",
    "DATEVALUE",
    "DAY",
    "EOMONTH",
    "FALSE",
    "FIND",
    "FLOOR",
    "HLOOKUP",
    "IF",
    "IFERROR",
    "IFNA",
    "IFS",
    "INDEX",
    "INDIRECT",
    "ISBLANK",
    "ISERROR",
    "ISLOGICAL",
    "ISNUMBER",
    "ISTEXT",
    "LAMBDA",
    "LEFT",
    "LEN",
    "LET",
    "LOWER",
    "MATCH",
    "MAX",
    "MEDIAN",
    "MID",
    "MIN",
    "MOD",
    "MONTH",
    "NOT",
    "NOW",
    "OFFSET",
    "OR",
    "PERCENTILE",
    "PERCENTILE.EXC",
    "PERCENTILE.INC",
    "POWER",
    "PRODUCT",
    "PROPER",
    "RAND",
    "RANDBETWEEN",
    "REPLACE",
    "RIGHT",
    "ROUND",
    "SEARCH",
    "SQRT",
    "STDEV",
    "STDEV.P",
    "STDEV.S",
    "SUBSTITUTE",
    "SUM",
    "SUMIF",
    "SUMIFS",
    "SWITCH",
    "TEXT",
    "TEXTJOIN",
    "TODAY",
    "TRIM",
    "TRUE",
    "UPPER",
    "VALUE",
    "VAR",
    "VAR.P",
    "VAR.S",
    "VLOOKUP",
    "XLOOKUP",
    "YEAR",
];

pub fn is_supported_function(name: &str) -> bool {
    SUPPORTED_FUNCTION_NAMES
        .iter()
        .any(|supported| supported.eq_ignore_ascii_case(name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn catalog_is_sorted_and_unique() {
        let mut sorted = SUPPORTED_FUNCTION_NAMES.to_vec();
        sorted.sort_unstable();
        assert_eq!(SUPPORTED_FUNCTION_NAMES, sorted.as_slice());

        let unique: HashSet<_> = SUPPORTED_FUNCTION_NAMES.iter().collect();
        assert_eq!(unique.len(), SUPPORTED_FUNCTION_NAMES.len());
    }

    #[test]
    fn catalog_contains_specialized_and_reference_functions() {
        for name in ["INDIRECT", "OFFSET", "XLOOKUP", "TEXTJOIN", "VAR.S"] {
            assert!(is_supported_function(name), "missing {name}");
        }
        assert!(is_supported_function("xlookup"));
        assert!(!is_supported_function("NOT_A_FORMULA"));
    }
}
