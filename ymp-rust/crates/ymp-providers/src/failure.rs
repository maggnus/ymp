//! Conservative typed failures; arbitrary connection messages are not a diagnosis.
use ymp_core::FailureClass;
#[derive(Debug)]
pub struct NativeFailure {
    pub class: FailureClass,
    /// An allowlisted protocol code, never a raw error payload.
    pub code: Option<String>,
}
impl std::fmt::Display for NativeFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Native invocation failed: {:?}", self.class)
    }
}
impl std::error::Error for NativeFailure {}
pub fn classify_failure(error: &anyhow::Error) -> (FailureClass, Option<String>) {
    if let Some(failure) = error.downcast_ref::<NativeFailure>() {
        let code = failure.code.as_deref().filter(|s| {
            matches!(
                *s,
                "429"
                    | "401"
                    | "403"
                    | "503"
                    | "overloaded"
                    | "rate_limit_exceeded"
                    | "insufficient_quota"
                    | "model_not_found"
                    | "authentication_error"
            )
        });
        return (failure.class, code.map(str::to_owned));
    }
    if error.is::<crate::NativeOutputLimit>() {
        return (
            FailureClass::MalformedResponse,
            Some("native_output_limit".into()),
        );
    }
    if let Some(io) = error.downcast_ref::<std::io::Error>() {
        use std::io::ErrorKind::*;
        return (
            match io.kind() {
                ConnectionReset | ConnectionAborted | BrokenPipe | Interrupted => {
                    FailureClass::TransientTransport
                }
                TimedOut => FailureClass::Timeout,
                _ => FailureClass::Unknown,
            },
            None,
        );
    }
    let text = error.to_string();
    let class = if text.starts_with("Provider turn timed out") {
        FailureClass::Timeout
    } else if text.starts_with("Turn cancelled;") {
        FailureClass::Cancelled
    } else if text.starts_with("unsupported_model:") || text.starts_with("unsupported_effort:") {
        FailureClass::UnsupportedConfiguration
    } else {
        FailureClass::Unknown
    };
    (class, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generic_connection_error_stays_unknown_and_native_codes_are_allowlisted() {
        assert_eq!(
            classify_failure(&anyhow::anyhow!("Connection failed; perhaps quota")),
            (FailureClass::Unknown, None)
        );
        assert_eq!(
            classify_failure(
                &NativeFailure {
                    class: FailureClass::QuotaExhausted,
                    code: Some("insufficient_quota".into())
                }
                .into()
            ),
            (
                FailureClass::QuotaExhausted,
                Some("insufficient_quota".into())
            )
        );
        assert_eq!(
            classify_failure(
                &NativeFailure {
                    class: FailureClass::Authentication,
                    code: Some("secret-native-payload".into())
                }
                .into()
            ),
            (FailureClass::Authentication, None)
        );
    }
}
