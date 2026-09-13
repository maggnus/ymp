#!/usr/bin/env python3
"""Inference-free Codex configuration and artificial-file permission probe.

Never calls turn/start, exec, review, or a model endpoint. Config responses are
filtered in memory; neither their raw bytes nor server stderr are persisted.
The installed provider retains its own authentication and configuration files.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import queue
import shutil
import subprocess
import tempfile
import threading
import time


CONTROLS = {
    "features.multi_agent": False,
    "features.multi_agent_v2": False,
    "agents.enabled": False,
    "features.memories": False,
    "memories.generate_memories": False,
    "memories.use_memories": False,
    "features.external_agent_memory_import": False,
    "features.hooks": False,
    "features.plugins": False,
    "features.apps": False,
    "features.browser_use": False,
    "features.computer_use": False,
    "features.image_generation": False,
    "features.goals": False,
    "features.skill_search": False,
    "features.skip_host_skill_discovery": True,
    "features.shell_snapshot": False,
    "features.unbounded_connection_retries": False,
    "approval_policy": "never",
    "approvals_reviewer": "user",
    "allow_login_shell": False,
    "model_reasoning_effort": "low",
    "project_doc_max_bytes": 0,
    "project_root_markers": [],
    "developer_instructions": "",
    "web_search": "disabled",
}
ALLOWED_METHODS = {
    "initialize", "config/read", "experimentalFeature/list", "model/list",
    "thread/start", "thread/unload", "command/exec",
}


def overrides(values):
    def toml(value):
        if isinstance(value, dict):
            return "{" + ",".join(json.dumps(key) + "=" + toml(item) for key, item in value.items()) + "}"
        return json.dumps(value, separators=(",", ":"))

    result = []
    for key, value in values.items():
        result.extend(["-c", key + "=" + toml(value)])
    return result


def pick(config, dotted):
    value = config
    for key in dotted.split("."):
        if not isinstance(value, dict) or key not in value:
            return None
        value = value[key]
    return value


def provider_args(codex, visible, denied_paths, read_only=False):
    """Arguments before production RpcProcess appends app-server --stdio.

    The trusted consumer must keep the original requested access in its journal.
    On macOS the inner legacy policy must be dangerFullAccess because a second
    seatbelt application fails. The outer native profile enforces original access.
    """
    profile = "ymp201_read" if read_only else "ymp201_write"
    permission = {
        "permissions." + profile + ".filesystem": {
            ":root": "write", **{str(path): "deny" for path in denied_paths},
            str(visible): "read" if read_only else "write",
        },
        "permissions." + profile + ".network.enabled": True,
    }
    return [*overrides(permission), "sandbox", "-P", profile, "-C", str(visible),
            "--", codex, *overrides(CONTROLS)]


class Rpc:
    def __init__(self, argv, cwd, start_new_session=True):
        self.owns_process_group = start_new_session
        self.process = subprocess.Popen(
            argv, cwd=cwd, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL, text=True, start_new_session=start_new_session,
        )
        self.messages = queue.Queue()
        self.counter = 0
        self.methods = []
        threading.Thread(target=self._read, daemon=True).start()
        try:
            self.call("initialize", {
                "clientInfo": {"name": "ymp201_controls_probe", "version": "2"},
                "capabilities": {"experimentalApi": True},
            })
        except Exception:
            self.close()
            raise
        self.process.stdin.write(json.dumps({"method": "initialized", "params": {}}) + "\n")
        self.process.stdin.flush()

    def _read(self):
        for line in self.process.stdout:
            try:
                self.messages.put(json.loads(line))
            except json.JSONDecodeError:
                self.messages.put({"probe_error": "non_json_protocol_output"})
        self.messages.put({"probe_error": "server_closed_output"})

    def call(self, method, params):
        if method not in ALLOWED_METHODS:
            raise ValueError("Non-allowlisted RPC is forbidden")
        self.counter += 1
        self.methods.append(method)
        self.process.stdin.write(json.dumps({
            "id": self.counter, "method": method, "params": params,
        }) + "\n")
        self.process.stdin.flush()
        deadline = time.monotonic() + 25
        while True:
            item = self.messages.get(timeout=max(0.1, deadline - time.monotonic()))
            if "probe_error" in item:
                raise RuntimeError(item["probe_error"])
            if item.get("id") == self.counter and "method" not in item:
                return item
            if "id" in item and "method" in item:
                # Never approve a server-initiated operation in this probe.
                self.process.stdin.write(json.dumps({
                    "id": item["id"], "error": {"code": -32601, "message": "probe rejects server requests"},
                }) + "\n")
                self.process.stdin.flush()
            if time.monotonic() > deadline:
                raise TimeoutError("allowlisted RPC deadline exceeded")

    def close(self):
        import signal
        if self.process.poll() is not None:
            return
        if not self.owns_process_group:
            self.process.terminate()
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait(timeout=5)
            return
        try:
            os.killpg(self.process.pid, signal.SIGTERM)
            self.process.wait(timeout=5)
        except ProcessLookupError:
            pass
        except PermissionError:
            self.process.terminate()
            self.process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            os.killpg(self.process.pid, signal.SIGKILL)
            self.process.wait(timeout=5)


def verify_report(report):
    """Check observations, including controls that deliberately expose canaries."""
    rows = {row["kind"]: row for row in report["runs"]}

    def values(kind, policy):
        for command in rows.get(kind, {}).get("commands", []):
            if command["requested_policy"] == policy:
                result = command["response"].get("result", {})
                if result.get("exitCode") != 0:
                    return {}
                return dict(line.split("=", 1) for line in result.get("stdout", "").splitlines() if "=" in line)
        return {}

    denied = {"hidden_read": "1", "hidden_write": "1", "peer_read": "1", "peer_write": "1"}
    checks = {}
    for kind, write_status in [("outer_native_sandbox", "0"), ("outer_native_readonly", "1")]:
        observed = values(kind, "dangerFullAccess")
        expected = {**denied, "visible_read": "0", "visible_write": write_status,
                    "readonly_read": "0", "readonly_write": "1"}
        checks[kind + "_enforces_access"] = observed == expected
    checks["direct_named_enforces_denials"] = all(
        values("direct", "named").get(key) == expected for key, expected in denied.items())
    checks["legacy_write_negative_control_exposes_hidden_and_peer"] = all(
        values("direct", "dangerFullAccess").get(key) == "0" for key in denied)
    checks["legacy_read_negative_control_exposes_hidden_and_peer"] = all(
        values("direct", "readOnly").get(key) == "0" for key in ["hidden_read", "peer_read"])
    checks["selected_controls_interpreted"] = all(
        row.get("config_selected") == CONTROLS for row in report["runs"])
    checks["thread_controls_and_low_confirmed"] = all(
        thread.get("error") is None
        and thread.get("selected_response", {}).get("reasoningEffort") == "low"
        and thread.get("selected_response", {}).get("instructionSources") == []
        and all(not feature["enabled"] for feature in thread.get("features_selected", [])
                if feature["name"] in ("multi_agent", "multi_agent_v2", "memories"))
        for row in report["runs"] for thread in row.get("threads", []))
    checks["project_instruction_positive_control_loads_canary"] = len(
        rows.get("direct", {}).get("instruction_positive_control", {}).get("instructionSources", [])) == 1
    return checks


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    options = parser.parse_args()
    codex = shutil.which("codex")
    if not codex:
        raise SystemExit("Installed codex executable is required")
    report = {"schema_version": 2, "inference_calls": 0, "probe_kind": "native_configuration_only"}
    report["version"] = subprocess.check_output([codex, "--version"], text=True).strip()
    report["binary"] = str(Path(codex).resolve())
    report["binary_sha256"] = hashlib.sha256(Path(codex).resolve().read_bytes()).hexdigest()
    report["requested_controls"] = CONTROLS
    with tempfile.TemporaryDirectory(prefix="ymp201-native-controls-v2-") as tmp:
        root = Path(tmp).resolve()
        visible, hidden, peer = (root / name for name in ("visible", "hidden", "peer"))
        for directory in (visible, hidden, peer):
            directory.mkdir()
            (directory / "canary.txt").write_text("artificial-canary\n")
        (visible / "AGENTS.md").write_text("Artificial project instruction canary. Never a model prompt.\n")
        schemas = root / "schema"
        completed = subprocess.run(
            [codex, "app-server", "generate-json-schema", "--experimental", "--out", str(schemas)],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=25,
        )
        if completed.returncode:
            raise SystemExit("Schema generation failed")
        selected = ["ThreadStartParams.json", "ThreadStartResponse.json", "CommandExecParams.json",
                    "ConfigReadParams.json", "ConfigReadResponse.json", "ExperimentalFeatureListParams.json"]
        report["schema_sha256"] = {}
        report["schema_properties"] = {}
        for name in selected:
            path = next(schemas.rglob(name))
            report["schema_sha256"][name] = hashlib.sha256(path.read_bytes()).hexdigest()
            schema = json.loads(path.read_text())
            report["schema_properties"][name] = schema.get("properties", {})
        feature_run = subprocess.run(
            [codex, *overrides(CONTROLS), "features", "list"],
            stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, timeout=25,
        )
        report["features_list_exit"] = feature_run.returncode
        feature_names = {key.split(".", 1)[1] for key in CONTROLS if key.startswith("features.")}
        report["features_list_selected"] = {
            line.split()[0]: line.split()[-1] == "true"
            for line in feature_run.stdout.splitlines()
            if line.split() and line.split()[0] in feature_names
        }
        permission = {
            "permissions.ymp201_probe.filesystem": {
                ":root": "write", str(hidden): "deny", str(peer): "deny",
                str(visible): "write", str(visible / "readonly.txt"): "read",
            },
            "permissions.ymp201_probe.network.enabled": True,
            "default_permissions": "ymp201_probe",
        }
        (visible / "readonly.txt").write_text("read-only-canary\n")
        canary_command = [
            "/bin/sh", "-c",
            'cat "$1/canary.txt" >/dev/null 2>&1; printf "visible_read=%s\\n" "$?"; '
            '(printf x >> "$1/canary.txt") 2>/dev/null; printf "visible_write=%s\\n" "$?"; '
            'cat "$2/canary.txt" >/dev/null 2>&1; printf "hidden_read=%s\\n" "$?"; '
            '(printf x >> "$2/canary.txt") 2>/dev/null; printf "hidden_write=%s\\n" "$?"; '
            'cat "$3/canary.txt" >/dev/null 2>&1; printf "peer_read=%s\\n" "$?"; '
            '(printf x >> "$3/canary.txt") 2>/dev/null; printf "peer_write=%s\\n" "$?"; '
            'cat "$1/readonly.txt" >/dev/null 2>&1; printf "readonly_read=%s\\n" "$?"; '
            '(printf x >> "$1/readonly.txt") 2>/dev/null; printf "readonly_write=%s\\n" "$?"',
            "canary", str(visible), str(hidden), str(peer),
        ]
        report["permission_template"] = permission
        base = [codex, *overrides(CONTROLS), *overrides(permission)]
        nested = [codex, *overrides(permission), "sandbox", "-P", "ymp201_probe", "-C", str(visible), "--", *base]
        report["argv_template"] = {"direct": base, "nested": nested}
        read_permission = {
            **permission,
            "permissions.ymp201_probe.filesystem": {
                **permission["permissions.ymp201_probe.filesystem"], str(visible): "read",
            },
        }
        readonly_nested = [codex, *overrides(read_permission), "sandbox", "-P", "ymp201_probe",
                           "-C", str(visible), "--", *base]
        report["argv_template"]["nested_readonly"] = readonly_nested
        report["consumer_provider_args"] = {
            "read_only": provider_args(codex, visible, [hidden, peer], True),
            "write": provider_args(codex, visible, [hidden, peer], False),
        }
        report["runs"] = []
        for label, prefix in [("direct", base), ("outer_native_sandbox", nested),
                              ("outer_native_readonly", readonly_nested)]:
            rpc = None
            row = {"kind": label}
            try:
                rpc = Rpc([*prefix, "app-server", "--stdio"], visible)
                raw = rpc.call("config/read", {"includeLayers": False, "cwd": str(visible)})
                config = raw.get("result", {}).get("config", {})
                row["config_selected"] = {key: pick(config, key) for key in CONTROLS}
                row["permissions_selected"] = {
                    "default_permissions": config.get("default_permissions"),
                    "probe_profile": config.get("permissions", {}).get("ymp201_probe"),
                    "legacy_sandbox_mode": config.get("sandbox_mode"),
                }
                # Names are only used internally to disable inherited MCP servers.
                mcp_disable = {key: {"enabled": False} for key in config.get("mcp_servers", {})}
                row["inherited_mcp_servers_disabled_count"] = len(mcp_disable)
                del raw, config
                models = rpc.call("model/list", {"includeHidden": True, "limit": 200})
                row["models_selected"] = [
                    {key: model.get(key) for key in ("id", "model", "supportedReasoningEfforts", "defaultReasoningEffort")}
                    for model in models.get("result", {}).get("data", [])
                    if model.get("model") in ("gpt-5.6-luna", "gpt-6-astra")
                ]
                row["commands"] = []
                for policy in ("named", "dangerFullAccess", "readOnly"):
                    params = {"command": canary_command, "cwd": str(visible), "timeoutMs": 5000, "outputBytesCap": 1000}
                    if policy == "named":
                        params["permissionProfile"] = "ymp201_probe"
                    else:
                        params["sandboxPolicy"] = {"type": policy}
                    response = rpc.call("command/exec", params)
                    row["commands"].append({"requested_policy": policy, "response": response})
                row["threads"] = []
                for policy in ("named", "danger-full-access", "read-only"):
                    params = {
                        "cwd": str(visible), "model": "gpt-5.6-luna", "ephemeral": True,
                        "approvalPolicy": "never", "approvalsReviewer": "user",
                        "config": {"mcp_servers": mcp_disable},
                    }
                    params["permissions" if policy == "named" else "sandbox"] = "ymp201_probe" if policy == "named" else policy
                    response = rpc.call("thread/start", params)
                    result = response.get("result", {})
                    row["threads"].append({
                        "requested_policy": policy,
                        "selected_response": {key: result.get(key) for key in (
                            "sandbox", "activePermissionProfile", "reasoningEffort", "model",
                            "instructionSources", "approvalPolicy", "approvalsReviewer",
                        )},
                        "error": response.get("error"),
                    })
                    thread_id = result.get("thread", {}).get("id")
                    if thread_id:
                        features = rpc.call("experimentalFeature/list", {"threadId": thread_id, "limit": 200})
                        row["threads"][-1]["features_selected"] = [
                            {key: item.get(key) for key in ("name", "enabled")}
                            for item in features.get("result", {}).get("data", [])
                            if item.get("name") in feature_names
                        ]
                        rpc.call("thread/unload", {"threadId": thread_id})
                if label == "direct":
                    # A positive control shows the source would otherwise load.
                    response = rpc.call("thread/start", {
                        "cwd": str(visible), "model": "gpt-5.6-luna", "ephemeral": True,
                        "approvalPolicy": "never", "sandbox": "read-only",
                        "config": {"mcp_servers": mcp_disable, "project_doc_max_bytes": 32768},
                    })
                    result = response.get("result", {})
                    row["instruction_positive_control"] = {
                        "instructionSources": result.get("instructionSources"),
                        "error": response.get("error"),
                    }
                    thread_id = result.get("thread", {}).get("id")
                    if thread_id:
                        rpc.call("thread/unload", {"threadId": thread_id})
                row["rpc_methods"] = rpc.methods
            except (RuntimeError, TimeoutError, queue.Empty, BrokenPipeError) as error:
                row["probe_error"] = type(error).__name__
            finally:
                if rpc:
                    rpc.close()
            report["runs"].append(row)
        report["limitations"] = [
            "No model turn was submitted; model tool availability is not inferred from a model response.",
            "The outer profile tests explicit hidden/peer denials, not a universal filesystem allowlist.",
            "Network was enabled to permit native provider operation; no claim of network isolation is made.",
            "Native authentication files were not read, copied, logged, or relocated by this script.",
            "A successful config/read is configuration interpretation evidence, not billed-usage evidence.",
        ]
        # Paths are artificial and replaced for portable review; no raw config survives.
        encoded = json.dumps(report, indent=2, sort_keys=True).replace(str(root), "<PROBE_ROOT>") + "\n"
        options.output.parent.mkdir(parents=True, exist_ok=True)
        options.output.write_text(encoded)
    print(json.dumps({"output": str(options.output), "inference_calls": 0,
                      "run_kinds": [row["kind"] for row in report["runs"]]}))


if __name__ == "__main__":
    main()
