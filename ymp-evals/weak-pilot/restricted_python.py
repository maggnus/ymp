"""Run one pilot Python check with a macOS filesystem allowlist.

Only copied candidate bytes and a caller-supplied probe enter the disposable
directory. No original candidate directory, observer source, private fixtures,
native state, inherited environment or credentials are exposed to its process.
This is the narrow local evaluation boundary, not a portable execution platform.
"""

import hashlib
import json
import math
from pathlib import Path
import subprocess
import sys
import sysconfig
import tempfile
import time


SANDBOX = Path("/usr/bin/sandbox-exec")
BOOTSTRAP = (
    "import runpy,sys; "
    "sys.path.insert(0,sys.argv[1]); "
    "sys.argv=sys.argv[2:]; "
    "runpy.run_path(sys.argv[0],run_name='__main__')"
)


class RestrictedExecutionError(ValueError):
    """A failed execution boundary, distinct from a failing public check."""

    def __init__(self, kind, message, *, process_terminated=False):
        super().__init__(message)
        self.kind = kind
        self.process_terminated = process_terminated


def executable():
    # Framework launchers use posix_spawn. Invoke their actual interpreter so
    # candidate code never needs permission to fork or launch another process.
    framework = Path(sys.base_prefix) / "Resources/Python.app/Contents/MacOS/Python"
    return (framework if framework.is_file() else Path(sys.executable)).resolve(strict=True)


def identity():
    runtime = executable()
    launcher = Path(sys.executable).resolve(strict=True)
    return {"backend": "macos-sandbox-exec", "python_executable": str(runtime),
            "python_sha256": hashlib.sha256(runtime.read_bytes()).hexdigest(),
            "python_launcher": str(launcher),
            "python_launcher_sha256": hashlib.sha256(launcher.read_bytes()).hexdigest(),
            "sandbox_executable": str(SANDBOX),
            "sandbox_sha256": hashlib.sha256(SANDBOX.read_bytes()).hexdigest(),
            "wrapper_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}


def profile(work):
    """Deny parent-directory enumeration as well as all non-runtime file reads."""
    runtime = executable()
    prefix = Path(sys.base_prefix).resolve(strict=True)
    stdlib = Path(sysconfig.get_path("stdlib")).resolve(strict=True)
    # macOS dyld requires the filesystem root itself. This literal does not
    # grant descendants: no user/private/peer parent directory can be listed.
    literals = {runtime, prefix / "Python", prefix / "Resources/Info.plist", Path("/"),
                Path("/dev/null"), Path("/dev/urandom"), Path("/dev/random")}
    trees = {work, stdlib, Path("/System/Library"), Path("/usr/lib")}
    ancestors = {parent for path in literals | trees for parent in path.parents}
    quote = lambda path: json.dumps(str(path))
    rule = lambda kind, paths: " ".join(f"({kind} {quote(path)})" for path in sorted(paths))
    return "\n".join([
        "(version 1)",
        "(deny default)",
        f"(allow process-exec (literal {quote(runtime)}))",
        "(allow sysctl-read)",
        f"(allow file-read* {rule('literal', literals)} {rule('subpath', trees)})",
        f"(allow file-read-metadata {rule('literal', ancestors)})",
        f"(deny file-read* (subpath {quote(stdlib / 'site-packages')}))",
        f"(allow file-write* (subpath {quote(work)}) (literal \"/dev/null\"))",
    ])


def _communicate(argv, work, environment, deadline, input_text=None):
    remaining = deadline - time.monotonic()
    if remaining <= 0:
        raise RestrictedExecutionError("timeout", "Restricted Python check exceeded its deadline before launch; no process remains",
                                       process_terminated=True)
    try:
        process = subprocess.Popen(argv, cwd=work, env=environment, stdin=subprocess.PIPE,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                   text=True, errors="replace", close_fds=True)
    except OSError as error:
        raise RestrictedExecutionError("sandbox_refused", f"Restricted Python launch failed: {error}") from error
    try:
        stdout, stderr = process.communicate(input_text, timeout=remaining)
    except subprocess.TimeoutExpired as error:
        # Fork is denied. Inherit the trusted observer's process group so its
        # external cancellation also reaches this process; do not create a new
        # session that could outlive the observer's outer deadline.
        try:
            process.kill()
        except ProcessLookupError:
            pass
        process.communicate()
        raise RestrictedExecutionError("timeout", "Restricted Python check exceeded its deadline; process terminated",
                                       process_terminated=True) from error
    return subprocess.CompletedProcess(argv, process.returncode, stdout, stderr)


def run(script, files, *, args=(), input_text=None, timeout=5):
    """Return CompletedProcess; boundary refusal/timeout raises a typed error.

    A completed nonzero exit is a candidate/public-test failure. The sandbox
    startup is separately checked before candidate execution, using the same
    profile and deadline. This helper never invokes an agent provider.
    """
    if sys.platform != "darwin" or not SANDBOX.is_file():
        raise RestrictedExecutionError("unsupported_platform", "Candidate execution requires macOS sandbox-exec")
    if not isinstance(timeout, (int, float)) or not math.isfinite(timeout) or timeout <= 0:
        raise ValueError("timeout must be positive and finite")
    if not isinstance(script, bytes) or any(
        not isinstance(name, str) or Path(name).name != name or name in ("", ".", "..", "_probe.py")
        or not isinstance(content, bytes) for name, content in files.items()
    ):
        raise ValueError("The probe and each flat candidate file must contain bytes")
    deadline = time.monotonic() + timeout
    with tempfile.TemporaryDirectory(prefix="ymp-pilot-restricted-") as directory:
        work = Path(directory).resolve(strict=True)
        for name, content in files.items():
            (work / name).write_bytes(content)
        (work / "_probe.py").write_bytes(script)
        environment = {"TMPDIR": str(work), "PATH": "/usr/bin:/bin",
                       "LANG": "C.UTF-8", "LC_ALL": "C.UTF-8"}
        command = [str(SANDBOX), "-p", profile(work), str(executable()),
                   "-I", "-B", "-S"]
        try:
            startup = _communicate([*command, "-c",
                                    "import runpy,json,copy,importlib.util,pathlib,csv; print('restricted-python-ready')"],
                                   work, environment, deadline)
        except RestrictedExecutionError as error:
            raise RestrictedExecutionError("sandbox_refused", "Restricted Python startup failed: " + str(error),
                                           process_terminated=error.process_terminated) from error
        if startup.returncode != 0 or startup.stdout != "restricted-python-ready\n":
            raise RestrictedExecutionError("sandbox_refused", "Restricted Python startup failed: " + startup.stderr[-1000:])
        return _communicate([*command, "-c", BOOTSTRAP, str(work), "_probe.py", *args],
                            work, environment, deadline, input_text)
