#![forbid(unsafe_code)]

use ymp_evals::{Mutation, check};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let baseline = check(Mutation::None, 6);
    let negative_controls = [
        check(Mutation::MintCreationBudget, 6),
        check(Mutation::IgnoreLeaseFence, 6),
        check(Mutation::AcceptQuiescence, 6),
    ];
    let passed = baseline.violation.is_none()
        && negative_controls
            .iter()
            .all(|report| report.violation.is_some());
    let output = serde_json::json!({
        "schema_version": 1,
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
