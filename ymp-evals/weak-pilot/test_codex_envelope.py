"""Offline tests of the narrow permission envelope; no installed model runs."""

import copy
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from codex_envelope import CONTROLS, overrides, parse_production_suffix, profile_for, read_policy, rewrite


class EnvelopeTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="ymp201-envelope-test-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        for name in ("visible", "hidden", "native", "controller"):
            (self.root / name).mkdir()
        self.policy = {"schema_version": 2, "controls": CONTROLS,
                       "deny_paths": [str(self.root / "hidden"), str(self.root / "controller")],
                       "native_home": str(self.root / "native")}
        self.request = {"id": 27, "method": "thread/start", "params": {
            "cwd": str(self.root / "visible"), "sandbox": "read-only", "model": "fixture-model",
            "developerInstructions": "A preserved artificial prompt.", "approvalPolicy": "never",
        }}

    def test_read_access_and_native_state_denial(self):
        name, profile = profile_for(self.policy, self.request["params"]["cwd"], "read-only")
        self.assertTrue(name.startswith("ymp201_read_"))
        self.assertEqual(profile["filesystem"][str(self.root / "visible")], "read")
        self.assertEqual(profile["filesystem"][str(self.root / "native")], "deny")
        self.assertFalse(profile["network"]["enabled"])

    def test_write_access_is_limited_to_working_directory(self):
        _, profile = profile_for(self.policy, self.request["params"]["cwd"], "danger-full-access")
        self.assertEqual(profile["filesystem"][":root"], "read")
        self.assertEqual(profile["filesystem"][str(self.root / "visible")], "write")
        self.assertEqual(profile["filesystem"][str(self.root / "hidden")], "deny")

    def test_only_control_fields_change(self):
        original = copy.deepcopy(self.request)
        changed, safe = rewrite(self.request, self.policy)
        self.assertEqual(self.request, original)
        params = changed["params"]
        self.assertNotIn("sandbox", params)
        self.assertIn("permissions", params)
        self.assertNotIn("developer_instructions", params["config"])
        for key, value in original["params"].items():
            if key != "sandbox":
                self.assertEqual(params[key], value)
        self.assertNotIn("preserved artificial prompt", json.dumps(safe))
        self.assertNotIn("fixture-model", json.dumps(safe))

    def test_resume_uses_same_mapping_and_preserves_thread_id(self):
        self.request["method"] = "thread/resume"
        self.request["params"]["threadId"] = "artificial-context-7"
        transformed, _ = rewrite(self.request, self.policy)
        self.assertEqual(transformed["params"]["threadId"], "artificial-context-7")

    def test_non_thread_requests_pass_unchanged(self):
        for method in ("turn/start", "turn/interrupt", "command/exec", "initialize"):
            original = {"method": method, "params": {"effort": "low", "text": "synthetic only"}}
            actual, journal = rewrite(original, self.policy)
            self.assertIs(actual, original)
            self.assertIsNone(journal)

    def test_unrecognized_access_fails(self):
        for access in (None, "workspace-write", "automatic"):
            self.request["params"]["sandbox"] = access
            with self.assertRaises(ValueError):
                rewrite(self.request, self.policy)

    def test_cannot_supply_both_permission_variants(self):
        self.request["params"]["permissions"] = ":danger-full-access"
        with self.assertRaises(ValueError):
            rewrite(self.request, self.policy)

    def test_protected_directory_cannot_be_solving_directory(self):
        self.request["params"]["cwd"] = str(self.root / "hidden")
        with self.assertRaises(ValueError):
            rewrite(self.request, self.policy)

    def test_controls_override_conflicting_thread_feature_flags(self):
        self.request["params"]["config"] = {"features": {"multi_agent": True, "memories": True}}
        actual, _ = rewrite(self.request, self.policy)
        self.assertFalse(actual["params"]["config"]["features"]["multi_agent"])
        self.assertFalse(actual["params"]["config"]["features"]["memories"])

    def test_policy_rejects_owner_impersonation_metadata(self):
        policy = dict(self.policy, owner_approved=True)
        path = self.root / "controller" / "policy.json"
        path.write_text(json.dumps(policy))
        with self.assertRaises(ValueError):
            read_policy(path)

    def test_inherited_mcp_names_do_not_create_incomplete_transports(self):
        actual, journal = rewrite(self.request, self.policy, ("quoted.name", "ymp"))
        servers = actual["params"]["config"]["mcp_servers"]
        self.assertEqual(servers, {})
        self.assertEqual(journal["inherited_mcp_servers_disabled_count"], 2)
        self.assertFalse(journal["trusted_ymp_mcp_enabled"])
        self.assertNotIn("quoted.name", json.dumps(journal))

    def endpoint(self):
        return {"command": "/artificial/ymp", "args": ["mcp", "stdio"],
                "env_vars": ["YMP_MCP_TOKEN"], "required": True,
                "default_tools_approval_mode": "approve"}

    def test_only_explicit_current_ymp_endpoint_is_enabled(self):
        self.request["params"]["config"] = {"mcp_servers": {"ymp": self.endpoint(), "other": {"enabled": True}}}
        actual, journal = rewrite(self.request, self.policy, ("ymp", "other"))
        servers = actual["params"]["config"]["mcp_servers"]
        self.assertEqual(servers["ymp"], {**self.endpoint(), "enabled": True})
        self.assertFalse(servers["other"]["enabled"])
        self.assertTrue(journal["trusted_ymp_mcp_enabled"])
        self.assertNotIn("/artificial/ymp", json.dumps(journal))

    def test_production_tail_matches_duplicate_endpoint(self):
        tail = ["app-server", "--stdio", *overrides({"mcp_servers.ymp." + key: value for key, value in self.endpoint().items()})]
        endpoint = parse_production_suffix(tail)
        self.request["params"]["config"] = {"mcp_servers": {"ymp": self.endpoint()}}
        actual, _ = rewrite(self.request, self.policy, ("ymp",), endpoint)
        self.assertTrue(actual["params"]["config"]["mcp_servers"]["ymp"]["enabled"])
        changed = dict(endpoint, command="/different/endpoint")
        with self.assertRaises(ValueError):
            rewrite(self.request, self.policy, (), changed)

    def test_tail_rejects_extra_configuration(self):
        with self.assertRaises(ValueError):
            parse_production_suffix(["app-server", "--stdio", "-c", "features.multi_agent=true"])

    def test_stdio_transparency_with_explicit_offline_echo_fixture(self):
        # This executable merely echoes bytes. It never imports a provider or
        # contacts a model; synthetic turn/start is passthrough test data.
        fake = self.root / "echo_fixture.py"
        fake.write_text(
            "import json,sys\nfor line in sys.stdin.buffer:\n"
            " message=json.loads(line)\n"
            " if message.get('method') in ('initialize','config/read'):\n"
            "  result={'config':{'mcp_servers':{'synthetic_unused':{'enabled':True}}}} if message['method']=='config/read' else {}\n"
            "  print(json.dumps({'id':message['id'],'result':result}),flush=True)\n"
            " elif message.get('method')!='initialized':\n"
            "  if message.get('method')=='thread/start': assert 'mcp_servers.synthetic_unused.enabled=false' in sys.argv\n"
            "  sys.stdout.buffer.write(line)\n  sys.stdout.buffer.flush()\n"
        )
        policy_path = self.root / "controller" / "policy.json"
        policy_path.write_text(json.dumps(self.policy))
        journal = self.root / "controller" / "journal.jsonl"
        unchanged = b'{"method":"turn/start", "params":{"effort":"low","input":[{"text":"fixture only"}]}}\n'
        usage = b'{"method":"thread/tokenUsage/updated","params":{"total":{"inputTokens":12,"outputTokens":4}}}\n'
        payload = json.dumps(self.request).encode() + b"\n" + unchanged + usage
        result = subprocess.run([
            sys.executable, str(Path(__file__).with_name("codex_envelope.py")),
            "--policy", str(policy_path), "--codex", sys.executable, "--codex-arg", str(fake),
            "--journal", str(journal), "app-server", "--stdio",
        ], input=payload, capture_output=True, timeout=10)
        self.assertEqual(result.returncode, 0, result.stderr)
        records = result.stdout.splitlines(keepends=True)
        self.assertEqual(records[1], unchanged)
        self.assertEqual(records[2], usage)
        changed = json.loads(records[0])
        self.assertEqual(changed["params"]["model"], "fixture-model")
        self.assertEqual(changed["params"]["developerInstructions"], self.request["params"]["developerInstructions"])
        self.assertEqual(len(journal.read_text().splitlines()), 1)


if __name__ == "__main__":
    unittest.main()
