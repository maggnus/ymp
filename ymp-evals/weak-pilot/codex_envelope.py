#!/usr/bin/env python3
"""Narrow research stdio envelope for existing Codex app-server requests.

Only thread/start and thread/resume control fields change. Prompts, model,
effort, IDs, responses, usage events, and all other requests pass unchanged.
Owner approval and executable/policy hashes belong to the trusted consumer.
"""

import argparse
import copy
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import threading
import time

from native_controls_probe import CONTROLS, Rpc, overrides


def read_policy(path):
    policy = json.loads(Path(path).read_text())
    if set(policy) != {"schema_version", "controls", "deny_paths", "native_home"}:
        raise ValueError("Policy fields do not match schema 2")
    if policy["schema_version"] != 2 or policy["controls"] != CONTROLS:
        raise ValueError("Policy controls do not match the verified native controls")
    if not isinstance(policy["deny_paths"], list) or not policy["deny_paths"]:
        raise ValueError("Policy must deny the controller and hidden artifact paths")
    for raw in [policy["native_home"], *policy["deny_paths"]]:
        if not isinstance(raw, str) or not Path(raw).is_absolute():
            raise ValueError("Policy paths must be absolute")
    return policy


def profile_for(policy, cwd, sandbox):
    if sandbox not in ("read-only", "danger-full-access"):
        raise ValueError("Unsupported original sandbox policy")
    if not isinstance(cwd, str) or not Path(cwd).is_absolute() or not Path(cwd).is_dir():
        raise ValueError("A current absolute working directory is required")
    cwd = str(Path(cwd).resolve())
    access = "read" if sandbox == "read-only" else "write"
    digest = hashlib.sha256((cwd + "\0" + access).encode()).hexdigest()[:16]
    name = "ymp201_" + access + "_" + digest
    # Root read supplies native tool/runtime files. It is not read isolation.
    # Exact protected subtrees deny the oracle, peer work, native state, and logs.
    filesystem = {":root": "read", cwd: access}
    protected = [policy["native_home"], *policy["deny_paths"]]
    for raw in protected:
        path = str(Path(raw).resolve())
        if Path(cwd).is_relative_to(path) or Path(path).is_relative_to(cwd):
            raise ValueError("Solving workspace and protected paths must not overlap")
        filesystem[path] = "deny"
    return name, {"filesystem": filesystem, "network": {"enabled": False}}


def merge_controls(config, values):
    for dotted, value in values.items():
        # Remove conflicting dotted spelling before installing nested TOML data.
        config.pop(dotted, None)
        node = config
        keys = dotted.split(".")
        for key in keys[:-1]:
            if key not in node:
                node[key] = {}
            if not isinstance(node[key], dict):
                raise ValueError("Existing thread config conflicts with required controls")
            node = node[key]
        node[keys[-1]] = copy.deepcopy(value)


def trusted_ymp_endpoint(value):
    """Accept only the existing production stdio endpoint shape."""
    keys = {"command", "args", "env_vars", "required", "default_tools_approval_mode"}
    if not isinstance(value, dict) or set(value) - (keys | {"enabled"}) or not keys.issubset(value):
        raise ValueError("Trusted ymp MCP endpoint is incomplete")
    if not isinstance(value["command"], str) or not value["command"]:
        raise ValueError("Trusted ymp MCP command is missing")
    if not isinstance(value["args"], list) or not all(isinstance(item, str) for item in value["args"]):
        raise ValueError("Trusted ymp MCP arguments are invalid")
    if value["env_vars"] != ["YMP_MCP_TOKEN"] or value["required"] is not True:
        raise ValueError("Trusted ymp MCP capability configuration is invalid")
    if value["default_tools_approval_mode"] != "approve":
        raise ValueError("Trusted ymp MCP approval configuration is invalid")
    return {**copy.deepcopy(value), "enabled": True}


def parse_production_suffix(trailing):
    if trailing[:2] != ["app-server", "--stdio"]:
        raise ValueError("Envelope requires the production app-server --stdio suffix")
    extras = trailing[2:]
    if not extras:
        return None
    endpoint = {}
    allowed = {"command", "args", "env_vars", "required", "default_tools_approval_mode"}
    if len(extras) != 2 * len(allowed):
        raise ValueError("Unexpected production provider overrides")
    for index in range(0, len(extras), 2):
        if extras[index] != "-c":
            raise ValueError("Unexpected production provider option")
        key, separator, raw = extras[index + 1].partition("=")
        if not separator or not key.startswith("mcp_servers.ymp."):
            raise ValueError("Only the production ymp endpoint can be supplied")
        field = key.removeprefix("mcp_servers.ymp.")
        if field not in allowed or field in endpoint:
            raise ValueError("Unexpected or duplicate ymp endpoint field")
        endpoint[field] = json.loads(raw)
    return trusted_ymp_endpoint(endpoint)


def inherited_mcp_names(options, controls):
    """Only names escape this metadata-only process; raw config is discarded."""
    rpc = Rpc([options.codex, *options.codex_arg, *overrides(controls), "app-server", "--stdio"],
              os.getcwd(), start_new_session=False)
    try:
        response = rpc.call("config/read", {"includeLayers": False, "cwd": os.getcwd()})
        result = response.get("result")
        if not isinstance(result, dict) or not isinstance(result.get("config"), dict):
            raise ValueError("Native MCP configuration could not be inspected")
        servers = result["config"].get("mcp_servers", {})
        if not isinstance(servers, dict) or not all(
            isinstance(name, str) and re.fullmatch(r"[A-Za-z0-9_-]+", name) for name in servers
        ):
            raise ValueError("Native MCP server names are invalid")
        names = tuple(servers)
        del servers, result, response
        return names
    finally:
        rpc.close()


def rewrite(message, policy, inherited_names=(), production_endpoint=None):
    if not isinstance(message, dict):
        raise ValueError("Client protocol records must be objects")
    method = message.get("method")
    if method not in ("thread/start", "thread/resume"):
        return message, None
    params = message.get("params")
    if not isinstance(params, dict) or "permissions" in params:
        raise ValueError("Thread must supply exactly the original legacy access policy")
    name, profile = profile_for(policy, params.get("cwd"), params.get("sandbox"))
    output = copy.deepcopy(message)
    target = output["params"]
    original = target.pop("sandbox")
    target["permissions"] = name
    config = target.setdefault("config", {})
    if not isinstance(config, dict):
        raise ValueError("Thread config must be an object")
    servers = config.setdefault("mcp_servers", {})
    if not isinstance(servers, dict):
        raise ValueError("Thread MCP configuration must be an object")
    explicit_ymp = servers.get("ymp")
    if explicit_ymp is not None:
        explicit_ymp = trusted_ymp_endpoint(explicit_ymp)
    if production_endpoint is not None:
        if explicit_ymp is not None and explicit_ymp != production_endpoint:
            raise ValueError("Conflicting trusted ymp endpoints")
        explicit_ymp = production_endpoint
    # Native thread.config replaces server entries. A generated entry containing
    # only enabled=false loses its transport and fails configuration parsing.
    # Inherited entries are disabled by scalar CLI overrides on process startup;
    # only complete, explicitly supplied thread entries are changed here.
    for server_name in servers:
        server = servers[server_name]
        if not isinstance(server, dict):
            raise ValueError("Thread MCP server configuration is invalid")
        server["enabled"] = False
    if explicit_ymp is not None:
        servers["ymp"] = copy.deepcopy(explicit_ymp)
    thread_controls = dict(policy["controls"])
    # Clear inherited user guidance once at process startup. The production
    # developerInstructions field remains the sole explicit role instruction;
    # do not introduce a competing thread.config.developer_instructions value.
    thread_controls.pop("developer_instructions")
    if "developer_instructions" in config:
        raise ValueError("Use the original explicit developerInstructions field")
    merge_controls(config, thread_controls)
    profiles = config.setdefault("permissions", {})
    if not isinstance(profiles, dict):
        raise ValueError("Existing permissions config is invalid")
    profiles[name] = profile
    safe = {
        "event": "native_permission_rewrite", "method": method,
        "requested_sandbox": original, "sent_permissions": name,
        "cwd": str(Path(params["cwd"]).resolve()),
        "profile_sha256": hashlib.sha256(json.dumps(profile, sort_keys=True).encode()).hexdigest(),
        "controls_sha256": hashlib.sha256(json.dumps(policy["controls"], sort_keys=True).encode()).hexdigest(),
        "native_home_denied_to_commands": True,
        "native_home_relocated": False,
        "inherited_mcp_servers_disabled_count": len(inherited_names),
        "trusted_ymp_mcp_enabled": explicit_ymp is not None,
    }
    return output, safe


def run_proxy(options, trailing):
    production_endpoint = parse_production_suffix(trailing)
    policy = read_policy(options.policy)
    journal_path = Path(options.journal)
    if not journal_path.is_absolute() or journal_path.exists():
        raise ValueError("Journal must be a new absolute controller-owned path")
    journal_path.parent.mkdir(parents=True, exist_ok=True)
    inherited_names = inherited_mcp_names(options, policy["controls"])
    disable_mcp = {"mcp_servers." + name + ".enabled": False for name in inherited_names}
    # Native auth and CODEX_HOME remain inherited. Never inspect or log them.
    process = subprocess.Popen(
        [options.codex, *options.codex_arg, *overrides(policy["controls"]),
         *overrides(disable_mcp), "app-server", "--stdio"],
        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
    )
    failures = []

    def output_copy():
        try:
            while block := process.stdout.read1(65536):
                sys.stdout.buffer.write(block)
                sys.stdout.buffer.flush()
        except (BrokenPipeError, OSError):
            process.terminate()

    def input_copy():
        try:
            with journal_path.open("x", encoding="utf-8") as journal:
                for raw in sys.stdin.buffer:
                    if len(raw) > 16 * 1024 * 1024:
                        raise ValueError("Client protocol record exceeds 16 MiB")
                    message = json.loads(raw)
                    rewritten, safe = rewrite(message, policy, inherited_names, production_endpoint)
                    if safe:
                        journal.write(json.dumps(safe, sort_keys=True) + "\n")
                        journal.flush()
                        raw = (json.dumps(rewritten, separators=(",", ":")) + "\n").encode()
                    process.stdin.write(raw)
                    process.stdin.flush()
            process.stdin.close()
        except (ValueError, OSError, TypeError):
            failures.append("Rejected invalid client control request")
            process.terminate()

    output_thread = threading.Thread(target=output_copy, daemon=True)
    input_thread = threading.Thread(target=input_copy, daemon=True)
    output_thread.start()
    input_thread.start()
    try:
        result = process.wait()
        output_thread.join(timeout=5)
        if failures:
            print(failures[0], file=sys.stderr)
            return 2
        return result
    finally:
        if process.poll() is None:
            process.terminate()


def probe(codex, output):
    """Exercise this proxy and native command sandbox without any model turn."""
    codex = str(Path(codex).resolve())
    report = {"kind": "native_envelope_without_inference", "inference_calls": 0,
              "standalone_command_profiles_equal_thread_profiles": True}
    report["version"] = subprocess.check_output([codex, "--version"], text=True).strip()
    report["binary_sha256"] = hashlib.sha256(Path(codex).read_bytes()).hexdigest()
    with tempfile.TemporaryDirectory(prefix="ymp201-envelope-probe-") as tmp:
        root = Path(tmp).resolve()
        visible, hidden, peer, state, controller = (root / name for name in (
            "visible", "hidden", "peer", "native-state", "controller"))
        for directory in (visible, hidden, peer, state, controller):
            directory.mkdir()
            (directory / "canary.txt").write_text("artificial-canary\n")
        policy = {"schema_version": 2, "controls": CONTROLS,
                  "deny_paths": [str(hidden), str(peer), str(state), str(controller)],
                  "native_home": str(Path(os.environ.get("CODEX_HOME", Path.home() / ".codex")).resolve())}
        policy_path, journal_path = controller / "policy.json", controller / "rewrite.jsonl"
        policy_path.write_text(json.dumps(policy))
        mcp_marker = controller / "mcp-starts.txt"
        mcp_fixture = controller / "artificial_mcp.py"
        mcp_fixture.write_text(
            "import json,sys\n"
            "with open(sys.argv[1],'a') as marker: marker.write(sys.argv[2]+'\\n')\n"
            "for line in sys.stdin:\n"
            " message=json.loads(line)\n"
            " if 'id' not in message: continue\n"
            " result={}\n"
            " if message['method']=='initialize': result={'protocolVersion':message['params']['protocolVersion'],'capabilities':{'tools':{}},'serverInfo':{'name':'artificial-control','version':'1'}}\n"
            " if message['method']=='tools/list': result={'tools':[]}\n"
            " print(json.dumps({'jsonrpc':'2.0','id':message['id'],'result':result}),flush=True)\n"
        )
        # command/exec is a standalone API and cannot reference a threadId. Load
        # exactly the same generated profiles at server startup for those probes.
        startup = {"sqlite_home": str(state), "log_dir": str(state / "logs"), "history.persistence": "none"}
        for artificial_name in ("inherited_canary", "ymp"):
            startup["mcp_servers." + artificial_name] = {
                "command": sys.executable, "args": [str(mcp_fixture), str(mcp_marker), "inherited"],
                "enabled": True,
            }
        for original in ("read-only", "danger-full-access"):
            name, profile = profile_for(policy, str(visible), original)
            startup["permissions." + name] = profile
            startup["default_permissions"] = name
        argv = [sys.executable, str(Path(__file__).resolve()), "--policy", str(policy_path),
                "--codex", codex, "--journal", str(journal_path)]
        for argument in overrides(startup):
            argv.extend(["--codex-arg=" + argument])
        argv.extend(["app-server", "--stdio"])
        rpc = Rpc(argv, str(visible))
        report["runs"] = []
        try:
            config = rpc.call("config/read", {"includeLayers": False, "cwd": str(visible)})
            safe_config = config.get("result", {}).get("config", {})
            report["state_config"] = {key: safe_config.get(key) for key in ("sqlite_home", "log_dir")}
            del config, safe_config
            for original in ("read-only", "danger-full-access"):
                name, _ = profile_for(policy, str(visible), original)
                thread_response = rpc.call("thread/start", {
                    "cwd": str(visible), "sandbox": original, "model": "gpt-5.6-luna",
                    "ephemeral": True, "approvalPolicy": "never",
                    "developerInstructions": "Artificial controls probe; no model turn is submitted.",
                })
                thread_result = thread_response.get("result", {})
                row = {"requested_sandbox": original, "expected_permissions": name,
                       "thread_selected": {key: thread_result.get(key) for key in (
                           "activePermissionProfile", "approvalPolicy", "reasoningEffort", "model", "instructionSources")},
                       "thread_error": thread_response.get("error")}
                command = [
                    "/bin/sh", "-c",
                    'cat "$1/canary.txt" >/dev/null 2>&1; printf "visible_read=%s\\n" "$?"; '
                    '(printf x >> "$1/canary.txt") 2>/dev/null; printf "visible_write=%s\\n" "$?"; '
                    'cat "$2/canary.txt" >/dev/null 2>&1; printf "hidden_read=%s\\n" "$?"; '
                    '(printf x >> "$2/canary.txt") 2>/dev/null; printf "hidden_write=%s\\n" "$?"; '
                    'cat "$3/canary.txt" >/dev/null 2>&1; printf "peer_read=%s\\n" "$?"; '
                    '(printf x >> "$3/canary.txt") 2>/dev/null; printf "peer_write=%s\\n" "$?"; '
                    'cat "$4/state_5.sqlite" >/dev/null 2>&1; printf "native_state_read=%s\\n" "$?"; '
                    '(printf x >> "$4/canary.txt") 2>/dev/null; printf "native_state_write=%s\\n" "$?"; '
                    'cat "$5/canary.txt" >/dev/null 2>&1; printf "controller_read=%s\\n" "$?"',
                    "canary", str(visible), str(hidden), str(peer), str(state), str(controller),
                ]
                response = rpc.call("command/exec", {
                    "command": command, "cwd": str(visible), "permissionProfile": name,
                    "timeoutMs": 5000, "outputBytesCap": 2000,
                })
                row["command_response"] = response
                result = response.get("result", {})
                values = dict(line.split("=", 1) for line in result.get("stdout", "").splitlines() if "=" in line)
                expected = {"visible_read": "0", "visible_write": "1" if original == "read-only" else "0",
                            "hidden_read": "1", "hidden_write": "1", "peer_read": "1", "peer_write": "1",
                            "native_state_read": "1", "native_state_write": "1", "controller_read": "1"}
                row["pass"] = result.get("exitCode") == 0 and values == expected and row["thread_error"] is None
                row["pass"] &= thread_result.get("activePermissionProfile", {}).get("id") == name
                report["runs"].append(row)
                thread_id = thread_result.get("thread", {}).get("id")
                if thread_id:
                    rpc.call("thread/unload", {"threadId": thread_id})
            report["native_host_wrote_own_state"] = (state / "state_5.sqlite").is_file()
            report["inherited_mcp_not_started"] = not mcp_marker.exists()
            report["rpc_methods"] = rpc.methods
        finally:
            rpc.close()
        report["rewrite_journal"] = [json.loads(line) for line in journal_path.read_text().splitlines()]
        endpoint = {
            "command": sys.executable, "args": [str(mcp_fixture), str(mcp_marker), "trusted"],
            "env_vars": ["YMP_MCP_TOKEN"], "required": True, "default_tools_approval_mode": "approve",
        }
        trusted_journal = controller / "trusted-rewrite.jsonl"
        trusted_argv = list(argv[:-2])
        trusted_argv[trusted_argv.index("--journal") + 1] = str(trusted_journal)
        trusted_argv.extend(["app-server", "--stdio", *overrides({
            "mcp_servers.ymp." + key: value for key, value in endpoint.items()
        })])
        trusted_rpc = Rpc(trusted_argv, str(visible))
        try:
            response = trusted_rpc.call("thread/start", {
                "cwd": str(visible), "sandbox": "read-only", "model": "gpt-5.6-luna",
                "ephemeral": True, "approvalPolicy": "never",
                "developerInstructions": "Artificial MCP controls probe; no model turn is submitted.",
                "config": {"mcp_servers": {"ymp": endpoint}},
            })
            result = response.get("result", {})
            report["trusted_mcp_thread_error"] = response.get("error")
            deadline = time.monotonic() + 3
            while not mcp_marker.exists() and time.monotonic() < deadline:
                time.sleep(0.05)
            report["artificial_mcp_launch_tags"] = mcp_marker.read_text().splitlines() if mcp_marker.exists() else []
            thread_id = result.get("thread", {}).get("id")
            if thread_id:
                trusted_rpc.call("thread/unload", {"threadId": thread_id})
            report["trusted_rpc_methods"] = trusted_rpc.methods
        finally:
            trusted_rpc.close()
        report["trusted_rewrite_journal"] = [json.loads(line) for line in trusted_journal.read_text().splitlines()]
        report["trusted_mcp_only_started"] = report["artificial_mcp_launch_tags"] == ["trusted"]
        report["pass"] = (all(row["pass"] for row in report["runs"])
                          and report["native_host_wrote_own_state"] and report["inherited_mcp_not_started"]
                          and report["trusted_mcp_only_started"] and report["trusted_mcp_thread_error"] is None)
        report["limitations"] = [
            "No model turn or model tool call was submitted.",
            "Standalone command/exec uses a server-loaded copy of exactly the profile selected on the thread.",
            "Only artificial native-state files were inspected; native credentials and existing histories were not read.",
            "Thread model and effort are metadata, not measured invocation usage.",
        ]
        encoded = json.dumps(report, indent=2).replace(str(root), "<PROBE_ROOT>") + "\n"
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(encoded)
    print(json.dumps({"output": str(output), "pass": report["pass"], "inference_calls": 0}))
    return 0 if report["pass"] else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--policy", type=Path)
    parser.add_argument("--codex", required=True)
    parser.add_argument("--journal", type=Path)
    parser.add_argument("--codex-arg", action="append", default=[])
    parser.add_argument("--probe", action="store_true")
    parser.add_argument("--output", type=Path)
    options, trailing = parser.parse_known_args()
    try:
        if options.probe:
            if trailing or not options.output:
                raise ValueError("Probe requires an output path and no provider suffix")
            return probe(options.codex, options.output)
        if not options.policy or not options.journal:
            raise ValueError("Policy and journal are required")
        return run_proxy(options, trailing)
    except (ValueError, OSError, json.JSONDecodeError):
        print("Native envelope configuration is invalid", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
