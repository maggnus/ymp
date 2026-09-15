//! Honest growth documentation (DEV-0005 acceptance criterion 10).

#![forbid(unsafe_code)]

use ymp_storage::STORAGE_DOCUMENTATION;

#[test]
fn storage_documentation_states_ownership_lifetime_and_deletion() {
    for required in [
        "journal root",
        "sessions",
        ".log",
        ".commit",
        "journal.lock",
        "owned by the user",
        "no format migration",
        "no compaction",
        "remove the root",
        "no hidden copies",
        "grow monotonically",
    ] {
        assert!(
            STORAGE_DOCUMENTATION.contains(required),
            "user-facing documentation is missing '{required}'"
        );
    }
}
