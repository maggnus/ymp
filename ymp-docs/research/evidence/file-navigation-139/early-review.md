# Early maintainer source review

The author is still implementing; these are draft observations, not final acceptance.

- Directory listings in the first browse.rs draft had no limit on consumed/stat-ed entries and sorted the entire result synchronously. Requested a bounded listing with an explicit incomplete reason, or a cancellable bounded implementation.
- display_name escaped control characters but left literal backslashes unchanged, allowing a literal backslash-n to share a caption with a newline. Requested filename-only backslash escaping, preserving literal source-code backslashes. Native action keys already remain distinct.
- cap-std relative handles, post-open regular-file checks, O_NONBLOCK on Unix, bounded preview text and pure syntax lookup from buffered data align with the read-only contract. These observations still require final tests and integration inspection.
