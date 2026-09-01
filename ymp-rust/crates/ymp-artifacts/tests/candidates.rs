use tempfile::tempdir;
use ymp_artifacts::{ArtifactError, ArtifactStore, Change, SubmissionManifest};
use ymp_storage::ObjectStore;

#[test]
fn private_workspace_produces_reproducible_immutable_candidate() {
    let temporary = tempdir().expect("temporary directory");
    let source = temporary.path().join("source");
    let workspace = temporary.path().join("workspace");
    let candidate_copy = temporary.path().join("candidate");
    std::fs::create_dir_all(source.join("src")).expect("source tree");
    std::fs::write(source.join("src/lib.rs"), b"pub fn value() -> u8 { 1 }\n")
        .expect("source file");
    std::fs::write(source.join("README.md"), b"base\n").expect("readme");

    let objects = ObjectStore::open(temporary.path().join("objects")).expect("object store");
    let artifacts = ArtifactStore::new(objects);
    let base = artifacts.capture_source(&source).expect("capture source");
    artifacts
        .materialize(&base.manifest_digest, &workspace)
        .expect("materialize private workspace");

    std::fs::write(
        workspace.join("src/lib.rs"),
        b"pub fn value() -> u8 { 2 }\n",
    )
    .expect("modify workspace");
    std::fs::remove_file(workspace.join("README.md")).expect("delete workspace file");
    std::fs::write(workspace.join("CHANGELOG.md"), b"candidate\n").expect("new workspace file");

    let submission = artifacts
        .create_submission(&base.manifest_digest, &workspace)
        .expect("create submission");
    assert_eq!(submission.change_count, 3);
    let candidate = artifacts
        .build_candidate(&base.manifest_digest, &submission.manifest_digest)
        .expect("build candidate");
    artifacts
        .materialize(&candidate.snapshot_digest, &candidate_copy)
        .expect("materialize candidate");

    assert_eq!(
        std::fs::read(candidate_copy.join("src/lib.rs")).expect("candidate source"),
        b"pub fn value() -> u8 { 2 }\n"
    );
    assert_eq!(
        std::fs::read(candidate_copy.join("CHANGELOG.md")).expect("candidate changelog"),
        b"candidate\n"
    );
    assert!(!candidate_copy.join("README.md").exists());

    let reproduced = artifacts
        .build_candidate(&base.manifest_digest, &submission.manifest_digest)
        .expect("reproduce candidate");
    assert_eq!(reproduced, candidate);
}

#[test]
fn submission_rejects_new_derived_files_without_storing_them() {
    let temporary = tempdir().expect("temporary directory");
    let source = temporary.path().join("source");
    let workspace = temporary.path().join("workspace");
    let candidate_copy = temporary.path().join("candidate");
    std::fs::create_dir_all(source.join("src")).expect("source tree");
    std::fs::create_dir_all(source.join("target")).expect("base target tree");
    std::fs::write(source.join("src/lib.rs"), b"base\n").expect("source file");
    std::fs::write(source.join("target/declared.txt"), b"keep\n").expect("declared target file");

    let objects = ObjectStore::open(temporary.path().join("objects")).expect("object store");
    let artifacts = ArtifactStore::new(objects.clone());
    let base = artifacts.capture_source(&source).expect("capture source");
    artifacts
        .materialize(&base.manifest_digest, &workspace)
        .expect("materialize workspace");
    std::fs::write(workspace.join("src/lib.rs"), b"candidate\n").expect("modify source");
    std::fs::write(workspace.join("target/build.bin"), b"derived\n").expect("write derived file");

    assert!(matches!(
        artifacts.create_submission_excluding(&base.manifest_digest, &workspace, &["target"]),
        Err(ArtifactError::ExcludedPathChanged(path)) if path == "target/build.bin"
    ));
    assert!(matches!(
        objects.verify(&ymp_domain::digest_bytes(b"derived\n")),
        Err(ymp_storage::ObjectStoreError::Missing(_))
    ));

    std::fs::remove_file(workspace.join("target/build.bin")).expect("remove derived file");
    let submission = artifacts
        .create_submission_excluding(&base.manifest_digest, &workspace, &["target"])
        .expect("create submission with unchanged exclusion");
    assert_eq!(submission.change_count, 1);
    let candidate = artifacts
        .build_candidate(&base.manifest_digest, &submission.manifest_digest)
        .expect("build candidate");
    artifacts
        .materialize(&candidate.snapshot_digest, &candidate_copy)
        .expect("materialize candidate");
    assert_eq!(
        std::fs::read(candidate_copy.join("target/declared.txt")).expect("retained base file"),
        b"keep\n"
    );
    assert!(!candidate_copy.join("target/build.bin").exists());
}

#[test]
fn submission_rejects_changes_to_base_files_inside_excluded_subtrees() {
    let temporary = tempdir().expect("temporary directory");
    let source = temporary.path().join("source");
    let workspace = temporary.path().join("workspace");
    std::fs::create_dir_all(source.join("target")).expect("base target tree");
    std::fs::write(source.join("source.txt"), b"base\n").expect("source file");
    std::fs::write(source.join("target/declared.txt"), b"keep\n").expect("declared target file");

    let objects = ObjectStore::open(temporary.path().join("objects")).expect("object store");
    let artifacts = ArtifactStore::new(objects);
    let base = artifacts.capture_source(&source).expect("capture source");
    artifacts
        .materialize(&base.manifest_digest, &workspace)
        .expect("materialize workspace");
    std::fs::write(workspace.join("target/declared.txt"), b"altered\n")
        .expect("modify excluded base file");

    assert!(matches!(
        artifacts.create_submission_excluding(&base.manifest_digest, &workspace, &["target"]),
        Err(ArtifactError::ExcludedPathChanged(path)) if path == "target/declared.txt"
    ));
}

#[test]
fn source_capture_can_omit_declared_derived_subtrees() {
    let temporary = tempdir().expect("temporary directory");
    let source = temporary.path().join("source");
    std::fs::create_dir_all(source.join("target/debug")).expect("create derived directory");
    std::fs::write(source.join("src.txt"), b"source\n").expect("write source");
    std::fs::write(source.join("target/debug/output"), b"derived\n").expect("write derived output");
    let objects = ObjectStore::open(temporary.path().join("objects")).expect("object store");
    let artifacts = ArtifactStore::new(objects);

    let snapshot = artifacts
        .capture_source_excluding(&source, &["target"])
        .expect("capture filtered source");
    let materialized = temporary.path().join("materialized");
    artifacts
        .materialize(&snapshot.manifest_digest, &materialized)
        .expect("materialize filtered source");

    assert_eq!(
        std::fs::read(materialized.join("src.txt")).expect("read source"),
        b"source\n"
    );
    assert!(!materialized.join("target").exists());
}

/// Product data that happens to stand inside the addressed source is never source material. Both
/// historical names are excluded at the root, while an equal name below a source directory stays
/// ordinary project content.
#[test]
fn source_capture_omits_product_roots_without_omitting_nested_project_names() {
    let temporary = tempdir().expect("temporary directory");
    let source = temporary.path().join("source");
    for directory in [".ymp", ".ymp-data", "fixtures/.ymp", "fixtures/.ymp-data"] {
        std::fs::create_dir_all(source.join(directory)).expect("source directory");
        std::fs::write(
            source.join(directory).join("sentinel"),
            directory.as_bytes(),
        )
        .expect("sentinel file");
    }
    std::fs::write(source.join("source.txt"), b"source\n").expect("source file");

    let objects = ObjectStore::open(temporary.path().join("objects")).expect("object store");
    let artifacts = ArtifactStore::new(objects);
    let snapshot = artifacts.capture_source(&source).expect("capture source");
    let materialized = temporary.path().join("materialized");
    artifacts
        .materialize(&snapshot.manifest_digest, &materialized)
        .expect("materialize source");

    assert!(!materialized.join(".ymp").exists());
    assert!(!materialized.join(".ymp-data").exists());
    assert_eq!(
        std::fs::read(materialized.join("fixtures/.ymp/sentinel")).expect("nested project file"),
        b"fixtures/.ymp"
    );
    assert_eq!(
        std::fs::read(materialized.join("fixtures/.ymp-data/sentinel"))
            .expect("nested project file"),
        b"fixtures/.ymp-data"
    );
}

#[test]
fn stale_base_is_rejected_without_candidate_creation() {
    let temporary = tempdir().expect("temporary directory");
    let source = temporary.path().join("source");
    let workspace = temporary.path().join("workspace");
    std::fs::create_dir_all(&source).expect("source tree");
    std::fs::write(source.join("file.txt"), b"base\n").expect("source file");
    let objects = ObjectStore::open(temporary.path().join("objects")).expect("object store");
    let artifacts = ArtifactStore::new(objects);
    let base = artifacts.capture_source(&source).expect("capture source");
    artifacts
        .materialize(&base.manifest_digest, &workspace)
        .expect("materialize workspace");
    std::fs::write(workspace.join("file.txt"), b"changed\n").expect("modify workspace");
    let submission = artifacts
        .create_submission(&base.manifest_digest, &workspace)
        .expect("create submission");

    assert!(matches!(
        artifacts.build_candidate(&"f".repeat(64), &submission.manifest_digest),
        Err(ArtifactError::BaseMismatch { .. })
    ));
}

#[cfg(unix)]
#[test]
fn source_symlink_is_rejected() {
    use std::os::unix::fs::symlink;

    let temporary = tempdir().expect("temporary directory");
    let source = temporary.path().join("source");
    std::fs::create_dir_all(&source).expect("source tree");
    symlink("../outside", source.join("escape")).expect("create symlink");
    let objects = ObjectStore::open(temporary.path().join("objects")).expect("object store");
    let artifacts = ArtifactStore::new(objects);

    assert!(matches!(
        artifacts.capture_source(&source),
        Err(ArtifactError::Symlink(_))
    ));
}

#[test]
fn forged_delete_and_unknown_schema_are_rejected() {
    let temporary = tempdir().expect("temporary directory");
    let source = temporary.path().join("source");
    std::fs::create_dir_all(&source).expect("source tree");
    std::fs::write(source.join("present.txt"), b"present\n").expect("source file");
    let objects = ObjectStore::open(temporary.path().join("objects")).expect("object store");
    let artifacts = ArtifactStore::new(objects.clone());
    let base = artifacts.capture_source(&source).expect("capture source");

    let forged = SubmissionManifest {
        schema_version: 1,
        base_snapshot_digest: base.manifest_digest.clone(),
        changes: vec![Change::Delete {
            path: "absent.txt".to_owned(),
        }],
    };
    let forged_digest = objects
        .put(&serde_json::to_vec(&forged).expect("serialize submission"))
        .expect("store submission");
    assert!(matches!(
        artifacts.build_candidate(&base.manifest_digest, &forged_digest),
        Err(ArtifactError::MissingDeleteTarget(path)) if path == "absent.txt"
    ));

    let future_snapshot = serde_json::json!({"schema_version": 2, "files": []});
    let future_digest = objects
        .put(&serde_json::to_vec(&future_snapshot).expect("serialize future snapshot"))
        .expect("store future snapshot");
    assert!(matches!(
        artifacts.load_snapshot(&future_digest),
        Err(ArtifactError::UnsupportedSchema {
            kind: "snapshot",
            actual: 2
        })
    ));
}

#[test]
fn forged_path_outside_workspace_is_rejected() {
    let temporary = tempdir().expect("temporary directory");
    let source = temporary.path().join("source");
    std::fs::create_dir_all(&source).expect("source tree");
    std::fs::write(source.join("present.txt"), b"present\n").expect("source file");
    let objects = ObjectStore::open(temporary.path().join("objects")).expect("object store");
    let artifacts = ArtifactStore::new(objects.clone());
    let base = artifacts.capture_source(&source).expect("capture source");
    let forged = SubmissionManifest {
        schema_version: 1,
        base_snapshot_digest: base.manifest_digest.clone(),
        changes: vec![Change::Delete {
            path: "../outside.txt".to_owned(),
        }],
    };
    let forged_digest = objects
        .put(&serde_json::to_vec(&forged).expect("serialize submission"))
        .expect("store submission");

    assert!(matches!(
        artifacts.build_candidate(&base.manifest_digest, &forged_digest),
        Err(ArtifactError::InvalidPath(path)) if path == "../outside.txt"
    ));
}
