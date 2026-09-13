# C3: restricted candidate execution

This is preparation evidence with zero model invocations. Original review
controls remain unchanged in the parent-retained `before`/`independent-round1`
records. The synthetic unrestricted control in this directory is a new paired
control: its deliberately unsafe public import reads an artificial canary, while
the actual selector and scorer deny that same read.

## Executable change

`restricted_python.run(script_bytes, files, args=(), input_text=None, timeout=5)`
is the shared boundary used by `public_select.py`, `observer.repair_checks`, and
the direct `repair_probe.py` command. Candidate files are copied as bytes into a
new flat directory; their original directories are never granted. The observer
retains private expected answers and passes only function names and arguments.
The direct probe CLI strips expected fields as well.

The local macOS `sandbox-exec` profile denies access by default. It grants the
disposable directory, Python standard-library/runtime files, and OS runtime
libraries. Site packages, network operations and process creation remain denied.
The subprocess environment contains only fixed locale/path values plus disposable
`HOME`/`TMPDIR`. No inherited environment values, credentials or native state are
read or copied. Python uses `-I -B -S`; framework installations launch the actual
interpreter rather than its process-spawning launcher.

The profile needs read access to the filesystem root **itself** for macOS dyld.
This literal rule does not grant root descendants. It may expose names immediately
under `/`; it does not expose user/private directories or permit their enumeration.
Both direct and symlink reads of artificial private, peer, native-state and
credential files are denied. Their immediate and common parent directories are
also denied. Other platforms refuse execution rather than falling back to an
unrestricted Python process. This is a local pilot boundary, not a portable or
universal operating-system isolation claim.

## Results and failure semantics

The [paired report](paired-canary-report.json) binds the helper, selector, observer,
probe, tests, Python launcher, actual Python interpreter and sandbox executable.
It records successful public and private checks after all attempted artificial
reads were refused. The [focused tests](focused-tests.log) also establish:

- Correct preparation and measured repair programs pass.
- A public assertion failure remains a valid negative check.
- A candidate timeout is followed by kill and wait, and remains a valid negative
  task outcome with `process_terminated=true`.
- Sandbox startup/refusal and unsupported platforms are measurement failures.
- The candidate cannot fork or bind a network socket.
- The direct repair-probe CLI uses the same restrictions.

`public_select.py` returns `measurement_valid`, per-candidate `execution_status`,
and `process_terminated` for timeouts. Its public selection rule is unchanged:
first public pass, otherwise first candidate. Completed negative public checks
do not consult private scoring. An execution-boundary failure returns CLI status
2. Observer scoring returns an unsuccessful objective result for ordinary
candidate errors/timeouts, and status 2 for a broken execution boundary.

The helper inherits the trusted observer's process group. The Rust consumer owns
external group cancellation and wait; the helper kills and waits for its sole
child on its own five-second check timeout. Candidate fork permission is denied.
The root verification covers the outer deadline and final combined consumer.

## Reproduce focused checks

```sh
python3 -B -m unittest discover -s ymp-evals/weak-pilot -p test_restricted_python.py -v
python3 -B -m unittest discover -s ymp-evals/tests -p test_weak_pilot.py -v
```

Both commands passed (10 new boundary tests and 10 existing pilot tests). No Cargo
chain, native control probe or inference was launched by this bounded C3 task;
the parent runs final combined checks and refreshes native control evidence.
