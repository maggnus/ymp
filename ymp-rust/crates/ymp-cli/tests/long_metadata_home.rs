//! A real executable consumer; every configured provider is deterministic and local.
use std::{fs, process::Command};
use ymp_core::{Config, ProviderKind};

#[test]
fn demo_preserves_long_metadata_home_and_selected_output_directory() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("long-metadata-directory-".repeat(7));
    let work = temp.path().join("selected-work");
    fs::create_dir_all(&work).unwrap();
    let mut config = Config::default();
    for provider in &mut config.providers {
        provider.kind = ProviderKind::Mock;
        provider.command = "internal".into();
        provider.args.clear();
        provider.env_refs.clear();
    }
    config.save(&home).unwrap();
    let sentinel = home.join("keep-this-metadata");
    fs::write(&sentinel, "unchanged").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_ymp"))
        .arg("--home")
        .arg(&home)
        .arg("-C")
        .arg(&work)
        .arg("demo")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "long-home demo failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(work.join("greeting.txt"))
            .unwrap()
            .trim(),
        "Hello from ymp"
    );
    assert!(home.join("state.sqlite").is_file());
    assert_eq!(fs::read_to_string(sentinel).unwrap(), "unchanged");
    assert!(!work.join(".git").exists());
    assert!(!home.join("greeting.txt").exists());
    assert_eq!(fs::read_dir(home.join("run")).unwrap().count(), 0);
}
