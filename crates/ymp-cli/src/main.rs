//! The ymp executable. Owner: W1-0015.
//! Created by owner decision on 2026-09-16; see ymp-docs/project-worktree.md.

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(ymp_cli::run(&arguments));
}
