use redoc_sheet_engine::{SheetCell, WorkbookModel};
use std::cmp::Ordering;
use std::process::ExitCode;
use std::time::Instant;

const FORMULA_ROWS: u32 = 10_000;
const WARM_ITERS: usize = 7;
const WARM_LIMIT_MS: f64 = 16.0;

fn median_ms(samples: &mut [f64]) -> f64 {
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
    let mid = samples.len() / 2;
    if samples.len().is_multiple_of(2) {
        (samples[mid - 1] + samples[mid]) / 2.0
    } else {
        samples[mid]
    }
}

fn main() -> ExitCode {
    let mut workbook = WorkbookModel::new_default();
    let sheet = &mut workbook.sheets[0];

    for row in 1..=FORMULA_ROWS {
        sheet.cells.insert(
            format!("{row}:1"),
            SheetCell {
                raw_value: row.to_string(),
                display_value: row.to_string(),
                formula: None,
                style: None,
            },
        );
        sheet.cells.insert(
            format!("{row}:2"),
            SheetCell {
                raw_value: format!("=A{row}*2"),
                display_value: "#EVAL...".to_string(),
                formula: Some(format!("=A{row}*2")),
                style: None,
            },
        );
    }

    let cold_started = Instant::now();
    workbook.recalculate(0);
    let cold_elapsed = cold_started.elapsed();

    let mut warm_samples_ms = Vec::with_capacity(WARM_ITERS);
    for _ in 0..WARM_ITERS {
        let warm_started = Instant::now();
        workbook.recalculate(0);
        warm_samples_ms.push(warm_started.elapsed().as_secs_f64() * 1000.0);
    }

    assert_eq!(workbook.sheets[0].cells["10000:2"].display_value, "20000");

    let warm_median = median_ms(&mut warm_samples_ms);
    println!(
        "grid-recalc: {FORMULA_ROWS} formulas cold={:.2?}, warm_median={warm_median:.2}ms (target: <{WARM_LIMIT_MS}ms)",
        cold_elapsed
    );

    if warm_median > WARM_LIMIT_MS {
        eprintln!(
            "perf budget failed: warm recalc median {warm_median:.2}ms exceeds {WARM_LIMIT_MS}ms"
        );
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}
