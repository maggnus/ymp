"""Apply one source mutation, run the named tests, restore the source and verify its hash."""
import hashlib
import json
import os
import pathlib
import subprocess
import sys

CRATE = pathlib.Path("/Users/maggnus/.paseo/worktrees/1ms2ynax/ymp146-recovery-ui/ymp-rust/crates/ymp-tui/src")
TARGET = "/Users/maggnus/.paseo/worktrees/1ms2ynax/ymp146-recovery-backend/target"

MUTATIONS = {
    "typed-team-edits-preferences": (
        "state.rs",
        '(Some(verb @ ("add" | "remove")), 3) if self.session.is_some() => {',
        '(Some(verb @ ("add" | "remove")), 3) if false && self.session.is_some() => {',
        ["tests::the_sidebar_shows_the_team_the_session_captured_not_the_edited_configuration",
         "tests::recovery::a_busy_member_is_replaced_during_a_run_without_touching_starting_preferences"],
    ),
    "stale-read-accepted": (
        "control.rs",
        "if generation != self.generation || self.session.as_deref() != Some(session) {",
        "if false {",
        ["tests::recovery::owner_work_stays_with_its_session_and_a_native_review_stops_with_the_window"],
    ),
    "stage-hold-ignored": (
        "state/owner.rs",
        "Some(RecoveryWaitReason::OwnerWait | RecoveryWaitReason::OwnerPause)",
        "Some(RecoveryWaitReason::OwnerWait | RecoveryWaitReason::OwnerPause) if false",
        ["tests::recovery::stage_holds_outlast_team_changes_and_unavailable_actions_name_their_cause"],
    ),
    "continuation-for-hidden-session": (
        "state/owner.rs",
        "                if shown {\n                    actions.push(Action::ContinueRun {",
        "                if true {\n                    actions.push(Action::ContinueRun {",
        ["tests::recovery::owner_work_stays_with_its_session_and_a_native_review_stops_with_the_window",
         "tests::recovery::a_saved_plan_is_reviewed_again_and_continued_with_current_files_from_the_team_page"],
    ),
}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    name, output = sys.argv[1], pathlib.Path(sys.argv[2])
    file, before, after, tests = MUTATIONS[name]
    path = CRATE / file
    original = path.read_bytes()
    original_hash = digest(path)
    text = original.decode()
    assert text.count(before) == 1, f"{name}: the mutated text is not unique"
    path.write_text(text.replace(before, after))
    results = {}
    try:
        for test in tests:
            run = subprocess.run(
                ["cargo", "test", "-p", "ymp-tui", "--lib", test, "--", "--exact"],
                cwd=CRATE.parents[2], capture_output=True, text=True,
                env={**os.environ, "CARGO_TARGET_DIR": TARGET})
            log = output / f"{name}--{test.rsplit('::', 1)[-1]}.log"
            log.write_text(run.stdout + run.stderr)
            passed = "test result: ok. 1 passed" in run.stdout
            failed = "test result: FAILED" in run.stdout
            panic = next((line for line in (run.stdout + run.stderr).splitlines()
                          if "panicked at" in line or line.startswith("error")), "")
            results[test] = {"exit": run.returncode, "passed": passed, "failed": failed,
                             "compiled": passed or failed, "first_failure": panic}
    finally:
        path.write_bytes(original)
    restored = digest(path) == original_hash
    report = {"mutation": name, "file": file, "original_sha256": original_hash,
              "restored": restored, "tests": results}
    (output / f"{name}.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))
    assert restored


if __name__ == "__main__":
    main()
