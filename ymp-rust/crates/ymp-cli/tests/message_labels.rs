//! Headless message headings through the real executable and a scripted ACP installation.
//!
//! Nothing here reaches a model. The installation is a script that answers the ACP protocol,
//! posts one team message per turn, describes its model with a caption and reports its effort.
use std::{path::Path, process::Command};
use ymp_storage::Store;

fn configure(home: &Path) {
    let fixture = format!(
        "{}/tests/fixtures/attribution_acp.py",
        env!("CARGO_MANIFEST_DIR")
    );
    // The profile names are the captions an installation returns, so a heading that falls back
    // to a configured name or to the actor identifier is caught either way.
    let config = format!(
        r#"version = 1
team = ["transport-one", "transport-two"]

[limits]
turns = 20
parallel = 2
attempts = 1
turn_timeout_secs = 10

[[providers]]
id = "fixture-provider"
kind = "acp"
command = "python3"
args = [{}]

[[agents]]
id = "transport-one"
name = "Default (recommended)"
provider = "fixture-provider"

[[agents]]
id = "transport-two"
name = "Recommended descriptive caption"
provider = "fixture-provider"
"#,
        toml::Value::String(fixture)
    );
    std::fs::write(home.join("config.toml"), config).unwrap();
}

#[test]
fn headless_headings_name_each_invocation_by_its_model_and_effort() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("home");
    let project = temp.path().join("project");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&project).unwrap();
    configure(&home);

    let output = Command::new(env!("CARGO_BIN_EXE_ymp"))
        .arg("--home")
        .arg(&home)
        .arg("--cwd")
        .arg(&project)
        .args([
            "run",
            "Report the fixture fact",
            "--no-memory",
            "--no-adaptive",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stdout}\n{stderr}");

    let headings: Vec<&str> = stderr
        .lines()
        .filter_map(|line| line.strip_prefix('[')?.strip_suffix(']'))
        .collect();
    assert!(
        headings.contains(&"you · user"),
        "the user's own heading changed: {headings:#?}"
    );
    let agents: Vec<&str> = headings
        .iter()
        .copied()
        .filter(|heading| !heading.starts_with("you · ") && !heading.starts_with("ymp · "))
        .collect();
    for kind in ["chat", "plan", "execute"] {
        assert!(
            agents
                .iter()
                .any(|heading| heading.ends_with(&format!(" · {kind}"))),
            "no agent heading of kind {kind}: {headings:#?}"
        );
    }
    for heading in &agents {
        assert!(
            heading.starts_with("native-model-z max · "),
            "a heading does not name its invocation's model and effort: {heading}"
        );
        for forbidden in ["transport-", "caption", "Default", "default"] {
            assert!(
                !heading.contains(forbidden),
                "a heading carries {forbidden:?} as a name: {heading}"
            );
        }
    }

    // Every stored message is presented, including what the run wrote just before it returned.
    let session = stdout
        .lines()
        .find_map(|line| line.strip_prefix("Session: "))
        .expect("the run names its session")
        .trim();
    let stored = Store::open(&home)
        .unwrap()
        .messages(session, 0, 10_000)
        .unwrap();
    assert_eq!(
        headings.len(),
        stored.len(),
        "headings {headings:#?} do not cover the stored messages {:#?}",
        stored
            .iter()
            .map(|message| (&message.author, &message.kind))
            .collect::<Vec<_>>()
    );
}
