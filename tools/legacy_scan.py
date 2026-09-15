#!/usr/bin/env python3
"""Reject code carried over from previous iterations.

Two checks run over the sources under the given paths (default: crates/):

1. Denylisted identifiers: names from the previous iteration's API that the
   approved model does not define. Their presence means a construct was
   copied instead of derived from the model.
2. Copied blocks: runs of normalized source lines that also appear in any
   legacy git tag (default: every tag matching ``legacy-*``). A run of at
   least ``--min-run`` normalized lines is reported with its legacy origin.

The script uses only the Python standard library and ``git``. It reads the
working tree and the tags; it modifies nothing.

Exit codes: 0 clean, 1 findings, 2 usage or environment error.
"""

from __future__ import annotations

import argparse
import hashlib
import os
import re
import subprocess
import sys
from collections.abc import Iterable

# Identifiers of the previous iterations (tags legacy-attempt-1, legacy-attempt-2,
# legacy-foundation) that have no counterpart in the approved model. Keep the list short and specific: the model's own names
# (Journal, Registry, Treasury, Gatekeeper, Assignment ...) are legitimate.
DENYLIST = (
    "CriterionEvaluation",
    "CriterionStatus",
    "SessionEvent",
    "LifecycleEventKind",
    "JournalEntry",
    "HistoryError",
    "DispatchError",
    "PolicyGatekeeper",
    "TrackedWorkspaceGuard",
    "LedgerTreasury",
    "SessionAccounting",
    "UsageAggregate",
    "ExecutionScenario",
    "CodexRegistry",
    "CodexProbe",
    "CodexStreamStats",
    "BuiltinCheckExecutor",
    "CheckExecutionError",
    "EvidenceFile",
    "VerifierDigest",
    "WorkspaceHoldConflict",
    "AdmissionSnapshot",
    "ObservationAccumulation",
    "HostEnforcement",
    "UncertaintyCause",
    "EffectState",
    "SessionCommand",
    "replay_execution",
    "record_effect_evidence",
    "observe_invocation",
    "start_invocation",
    "admit_assignment",
    "run_linux_check_sandbox",
    # attempt 2 (legacy-attempt-2)
    "RecordLinks",
    "TeamServer",
    "NativeContinuation",
    "PlanVersion",
    "RecoveryStage",
    "StoreLock",
    "AccessCoordinator",
    "GrantRecord",
    "BoundedRecoveryPolicy",
    "McpEndpoint",
    "BoardProposal",
    # attempt 1 (legacy-attempt-1)
    "CommitmentCommand",
    "LeaseRecord",
    "EventDigestInput",
    "SubmissionManifest",
    "CandidateIdentity",
    "VerifiedEvidence",
    "StoredVerificationEvidence",
    "RunKeepingAuthority",
    "McpBinding",
    "ToolHostProbeFailureEvidence",
    "ProviderRequestState",
    "ManagedShutdown",
    "FrozenPool",
    "CatalogRoute",
    "RecruitmentPolicy",
    "ParticipantStartPath",
    "ProbeTransportIdentity",
    "AgentToolCapabilities",
    "ExcludedPathChanged",
    "ApplyInterrupted",
    "unestablished_terminations",
    "absorb_committed",
    "recover_projection",
    "append_terminal",
    "blocking_ancestors",
)

SOURCE_SUFFIXES = (".rs", ".py")
SKIP_DIRS = {"target", ".git", "__pycache__"}
PUNCTUATION_ONLY = re.compile(r"^[{}()\[\];,]*$")
WHITESPACE = re.compile(r"\s+")
DENY_PATTERN = re.compile(r"\b(" + "|".join(map(re.escape, DENYLIST)) + r")\b")


def normalize(lines: Iterable[str]) -> list[tuple[int, str]]:
    """Return (original_line_number, normalized_text) for substantive lines."""
    result: list[tuple[int, str]] = []
    for number, raw in enumerate(lines, start=1):
        text = WHITESPACE.sub(" ", raw.strip())
        if not text or text.startswith(("//", "#!", "# ")) or text == "#":
            continue
        if PUNCTUATION_ONLY.match(text):
            continue
        result.append((number, text))
    return result


def window_key(texts: list[str]) -> str:
    return hashlib.blake2b("\n".join(texts).encode("utf-8"), digest_size=16).hexdigest()


def git(*args: str) -> str:
    completed = subprocess.run(
        ["git", *args], capture_output=True, text=True, check=False, encoding="utf-8", errors="replace"
    )
    if completed.returncode != 0:
        raise RuntimeError(completed.stderr.strip() or f"git {' '.join(args)} failed")
    return completed.stdout


def legacy_refs(explicit: str | None) -> list[str]:
    if explicit:
        return [ref for ref in explicit.split(",") if ref]
    return [line for line in git("tag", "--list", "legacy-*").splitlines() if line]


def legacy_windows(refs: list[str], min_run: int) -> dict[str, tuple[str, str, int]]:
    windows: dict[str, tuple[str, str, int]] = {}
    for ref in refs:
        paths = [
            path
            for path in git("ls-tree", "-r", "--name-only", ref).splitlines()
            if path.endswith(SOURCE_SUFFIXES) and not path.startswith("target/")
        ]
        for path in paths:
            normalized = normalize(git("show", f"{ref}:{path}").splitlines())
            texts = [text for _, text in normalized]
            for start in range(0, len(texts) - min_run + 1):
                key = window_key(texts[start : start + min_run])
                windows.setdefault(key, (ref, path, normalized[start][0]))
    return windows


def working_files(paths: list[str]) -> list[str]:
    files: list[str] = []
    for root in paths:
        if os.path.isfile(root):
            files.append(root)
            continue
        for directory, dirnames, filenames in os.walk(root):
            dirnames[:] = [d for d in dirnames if d not in SKIP_DIRS]
            for name in filenames:
                if name.endswith(SOURCE_SUFFIXES):
                    files.append(os.path.join(directory, name))
    return sorted(files)


def scan_denylist(path: str, lines: list[str]) -> list[str]:
    findings = []
    for number, line in enumerate(lines, start=1):
        for match in DENY_PATTERN.finditer(line):
            findings.append(f"{path}:{number}: legacy identifier `{match.group(1)}`")
    return findings


def scan_copies(path: str, lines: list[str], windows: dict[str, tuple[str, str, int]], min_run: int) -> list[str]:
    normalized = normalize(lines)
    texts = [text for _, text in normalized]
    findings = []
    run_start: int | None = None
    run_end = -1
    origin: tuple[str, str, int] | None = None

    def flush() -> None:
        if run_start is None or origin is None:
            return
        first_line = normalized[run_start][0]
        last_line = normalized[run_end + min_run - 1][0]
        count = run_end - run_start + min_run
        ref, legacy_path, legacy_line = origin
        findings.append(
            f"{path}:{first_line}-{last_line}: {count} normalized lines also in {ref}:{legacy_path}:{legacy_line}"
        )

    for start in range(0, len(texts) - min_run + 1):
        hit = windows.get(window_key(texts[start : start + min_run]))
        if hit is None:
            flush()
            run_start, origin = None, None
            continue
        if run_start is not None and start == run_end + 1:
            run_end = start
        else:
            flush()
            run_start, run_end, origin = start, start, hit
    flush()
    return findings


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("paths", nargs="*", default=["crates"], help="files or directories to scan (default: crates)")
    parser.add_argument("--min-run", type=int, default=12, help="normalized lines that count as a copied block (default: 12)")
    parser.add_argument("--refs", help="comma-separated legacy refs (default: every tag matching legacy-*)")
    parser.add_argument("--no-copies", action="store_true", help="run only the identifier denylist")
    args = parser.parse_args(argv)
    if args.min_run < 4:
        parser.error("--min-run must be at least 4")

    try:
        files = working_files([p for p in args.paths if os.path.exists(p)])
        windows: dict[str, tuple[str, str, int]] = {}
        refs: list[str] = []
        if not args.no_copies:
            refs = legacy_refs(args.refs)
            if refs:
                windows = legacy_windows(refs, args.min_run)
            else:
                print("legacy-scan: no legacy-* tag found; copied-block check skipped", file=sys.stderr)
    except RuntimeError as error:
        print(f"legacy-scan: {error}", file=sys.stderr)
        return 2

    findings: list[str] = []
    for path in files:
        with open(path, encoding="utf-8", errors="replace") as handle:
            lines = handle.read().splitlines()
        findings.extend(scan_denylist(path, lines))
        if windows:
            findings.extend(scan_copies(path, lines, windows, args.min_run))

    for finding in findings:
        print(finding)
    summary = f"legacy-scan: {len(files)} files, {len(refs)} legacy ref(s), {len(findings)} finding(s)"
    if findings:
        print(summary, file=sys.stderr)
        return 1
    print(summary)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
