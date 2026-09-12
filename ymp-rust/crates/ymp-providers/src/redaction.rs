//! Redact the runtime-issued team capability from errors leaving this crate.
//! Only the current request's known value is used; native credentials are never read.
use anyhow::Error;

pub(crate) fn team_capability(error: Error, capability: Option<&str>) -> Error {
    let Some(capability) = capability.filter(|value| !value.is_empty()) else {
        return error;
    };
    // Render every source before redacting. Wrapping the original error with a
    // sanitized context would still expose its sources through alternate/debug
    // formatting, chain(), or downstream persistence.
    let diagnostic = format!("{error:#}");
    let sanitized = diagnostic
        .replace(
            &format!("YMP_MCP_TOKEN={capability}"),
            "[redacted team capability]",
        )
        .replace(capability, "[redacted team capability]");
    if sanitized == diagnostic {
        // Preserve typed classifications when the error contains no capability.
        error
    } else {
        Error::msg(sanitized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;

    #[test]
    fn every_error_format_and_source_is_redacted_without_losing_diagnosis() {
        let capability = "synthetic-current-assignment-capability";
        let error = anyhow!("Transport rejected {capability}")
            .context(format!("MCP setup failed: YMP_MCP_TOKEN={capability}"))
            .context("initialize: native error -32000");
        let sanitized = team_capability(error, Some(capability));
        for display in [
            format!("{sanitized}"),
            format!("{sanitized:#}"),
            format!("{sanitized:?}"),
            format!("{sanitized:#?}"),
        ] {
            assert!(!display.contains(capability));
            assert!(!display.contains("YMP_MCP_TOKEN="));
            assert!(display.contains("initialize: native error -32000"));
            assert!(display.contains("MCP setup failed"));
            assert!(display.contains("Transport rejected"));
            assert!(display.contains("[redacted team capability]"));
        }
        assert!(sanitized
            .chain()
            .all(|cause| !cause.to_string().contains(capability)));
    }

    #[test]
    fn nested_protocol_error_data_and_repeated_values_are_redacted() {
        let capability = "synthetic-capability-2";
        let payload = serde_json::json!({
            "code":-32000,"message":capability,
            "data":{"causes":[{"context":format!("request used {capability} twice: {capability}")}]}
        });
        let error = team_capability(anyhow!("session/load: {payload}"), Some(capability));
        let display = format!("{error:#}");
        assert!(!display.contains(capability));
        assert!(display.contains("session/load") && display.contains("-32000"));
        assert_eq!(display.matches("[redacted team capability]").count(), 3);
    }

    #[test]
    fn safe_errors_keep_their_types_and_empty_capabilities_do_not_rewrite_text() {
        for capability in [None, Some(""), Some("unused-capability")] {
            let error = Error::new(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Access denied",
            ))
            .context("Native process unavailable");
            let safe = team_capability(error, capability);
            assert!(safe.downcast_ref::<std::io::Error>().is_some());
            assert_eq!(
                format!("{safe:#}"),
                "Native process unavailable: Access denied"
            );
        }
    }
}
