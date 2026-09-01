pub fn next_head(previous: &str, id: &str, total: u64) -> String {
    format!("{previous}>{id}:{total}")
}
