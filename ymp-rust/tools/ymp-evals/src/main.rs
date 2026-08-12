#![forbid(unsafe_code)]

use ymp_evals::{Mutation, check};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let baseline = check(Mutation::None);
    let negative_controls = [
        check(Mutation::SkipAwardReservation),
        check(Mutation::IgnoreLeaseFence),
        check(Mutation::AllowStaleObligationReturn),
        check(Mutation::AllowSecondObligationReturn),
        check(Mutation::ReapplyDuplicateCommand),
    ];
    let passed = baseline.passed
        && negative_controls
            .iter()
            .all(|report| !report.passed && !report.shortest_counterexample.is_empty());
    let output = serde_json::json!({
        "schema_version": 3,
        "passed": passed,
        "baseline": baseline,
        "negative_controls": negative_controls,
    });
    println!("{}", serde_json::to_string_pretty(&output)?);
    if !passed {
        return Err("protocol model validation failed".into());
    }
    Ok(())
}
