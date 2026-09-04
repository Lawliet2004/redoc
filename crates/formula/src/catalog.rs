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
    "CHOOSECOLS",
    "CHOOSEROWS",
    "COLUMNS",
    "CONCAT",
    "CONCATENATE",
    "CORREL",
    "COS",
    "COUNT",
    "COUNTA",
    "COUNTBLANK",
    "COUNTIF",
    "COUNTIFS",
    "DATE",
    "DATEVALUE",
    "DAY",
    "DAYS",
    "DROP",
    "EDATE",
    "EOMONTH",
    "EXP",
    "FALSE",
    "FILTER",
    "FIND",
    "FLOOR",
    "FV",
    "HLOOKUP",
    "HOUR",
    "HSTACK",
    "IF",
    "IFERROR",
    "IFNA",
    "IFS",
    "INDEX",
    "INDIRECT",
    "INT",
    "IRR",
    "ISBLANK",
    "ISERROR",
    "ISLOGICAL",
    "ISNUMBER",
    "ISTEXT",
    "LEFT",
    "LEN",
    "LET",
    "LN",
    "LOG",
    "LOG10",
    "LOWER",
    "MATCH",
    "MAX",
    "MAXIFS",
    "MEDIAN",
    "MID",
    "MIN",
    "MINIFS",
    "MINUTE",
    "MOD",
    "MONTH",
    "NETWORKDAYS",
    "NOT",
    "NOW",
    "NPV",
    "OFFSET",
    "OR",
    "PERCENTILE",
    "PERCENTILE.EXC",
    "PERCENTILE.INC",
    "PMT",
    "POWER",
    "PRODUCT",
    "PROPER",
    "PV",
    "QUARTILE",
    "QUARTILE.INC",
    "RAND",
    "RANDARRAY",
    "RANDBETWEEN",
    "RANK",
    "RANK.EQ",
    "REPLACE",
    "RIGHT",
    "ROUND",
    "ROUNDDOWN",
    "ROUNDUP",
    "ROWS",
    "SEARCH",
    "SECOND",
    "SEQUENCE",
    "SIGN",
    "SIN",
    "SORT",
    "SORTBY",
    "SQRT",
    "STDEV",
    "STDEV.P",
    "STDEV.S",
    "SUBSTITUTE",
    "SUM",
    "SUMIF",
    "SUMIFS",
    "SUMPRODUCT",
    "SUMSQ",
    "SWITCH",
    "TAKE",
    "TAN",
    "TEXT",
    "TEXTJOIN",
    "TIME",
    "TOCOL",
    "TODAY",
    "TOROW",
    "TRANSPOSE",
    "TRIM",
    "TRUE",
    "TRUNC",
    "UNIQUE",
    "UPPER",
    "VALUE",
    "VAR",
    "VAR.P",
    "VAR.S",
    "VLOOKUP",
    "VSTACK",
    "WEEKDAY",
    "WORKDAY",
    "WRAPCOLS",
    "WRAPROWS",
    "XLOOKUP",
    "XOR",
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
        for name in [
            "INDIRECT",
            "OFFSET",
            "XLOOKUP",
            "TEXTJOIN",
            "VAR.S",
            "MINUTE",
            "DAYS",
            "EDATE",
            "WORKDAY",
            "LET",
            "FILTER",
            "SORT",
            "SORTBY",
            "TOCOL",
            "TOROW",
            "UNIQUE",
            "WRAPROWS",
            "WRAPCOLS",
            "CORREL",
            "QUARTILE.INC",
            "RANK.EQ",
            "SUMSQ",
        ] {
            assert!(is_supported_function(name), "missing {name}");
        }
        assert!(is_supported_function("xlookup"));
        assert!(!is_supported_function("LAMBDA"));
        assert!(!is_supported_function("NOT_A_FORMULA"));
    }
}
