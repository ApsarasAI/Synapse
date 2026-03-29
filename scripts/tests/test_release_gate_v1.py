import os
import pathlib
import subprocess
import tempfile
import textwrap
import unittest


REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / "scripts" / "release_gate_v1.sh"


def _write_executable(path: pathlib.Path, content: str) -> None:
    path.write_text(content)
    path.chmod(0o755)


class ReleaseGateV1Tests(unittest.TestCase):
    def _run_gate(self, *, enable_perf_gate: bool) -> subprocess.CompletedProcess[str]:
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = pathlib.Path(tmp_dir)
            log_path = tmp_path / "calls.log"

            cargo_stub = tmp_path / "cargo-stub.sh"
            python_stub = tmp_path / "python-stub.sh"
            quickstart_stub = tmp_path / "quickstart-smoke.sh"
            pr_review_stub = tmp_path / "pr-review-smoke.sh"
            perf_stub = tmp_path / "perf-gate.sh"

            _write_executable(
                cargo_stub,
                textwrap.dedent(
                    f"""\
                    #!/usr/bin/env bash
                    set -euo pipefail
                    printf 'cargo:%s\\n' "$*" >> "{log_path}"
                    """
                ),
            )
            _write_executable(
                python_stub,
                textwrap.dedent(
                    f"""\
                    #!/usr/bin/env bash
                    set -euo pipefail
                    printf 'python:%s\\n' "$*" >> "{log_path}"
                    """
                ),
            )
            _write_executable(
                quickstart_stub,
                textwrap.dedent(
                    f"""\
                    #!/usr/bin/env bash
                    set -euo pipefail
                    printf 'quickstart\\n' >> "{log_path}"
                    """
                ),
            )
            _write_executable(
                pr_review_stub,
                textwrap.dedent(
                    f"""\
                    #!/usr/bin/env bash
                    set -euo pipefail
                    printf 'pr_review\\n' >> "{log_path}"
                    """
                ),
            )
            _write_executable(
                perf_stub,
                textwrap.dedent(
                    f"""\
                    #!/usr/bin/env bash
                    set -euo pipefail
                    printf 'perf\\n' >> "{log_path}"
                    """
                ),
            )

            env = os.environ.copy()
            env["SYNAPSE_CARGO_BIN"] = str(cargo_stub)
            env["SYNAPSE_PYTHON_BIN"] = str(python_stub)
            env["SYNAPSE_QUICKSTART_SMOKE_SCRIPT"] = str(quickstart_stub)
            env["SYNAPSE_PR_REVIEW_DEMO_SMOKE_SCRIPT"] = str(pr_review_stub)
            env["SYNAPSE_PERF_GATE_SCRIPT"] = str(perf_stub)
            env["SYNAPSE_RELEASE_RUN_PERF_GATE"] = "1" if enable_perf_gate else "0"

            result = subprocess.run(
                ["bash", str(SCRIPT)],
                cwd=REPO_ROOT,
                env=env,
                text=True,
                capture_output=True,
                check=False,
            )
            result.log = log_path.read_text() if log_path.exists() else ""
            return result

    def test_release_gate_skips_perf_gate_by_default(self):
        result = self._run_gate(enable_perf_gate=False)

        self.assertEqual(result.returncode, 0, msg=result.stderr)
        self.assertIn("perf gate skipped", result.stdout)
        self.assertIn("cargo:fmt --all --check", result.log)
        self.assertIn("cargo:clippy --workspace --all-targets -- -D warnings", result.log)
        self.assertIn("cargo:test --workspace", result.log)
        self.assertIn("python:-m unittest discover -s sdk/python/tests", result.log)
        self.assertIn("quickstart", result.log)
        self.assertIn("pr_review", result.log)
        self.assertNotIn("perf", result.log)

    def test_release_gate_runs_perf_gate_when_enabled(self):
        result = self._run_gate(enable_perf_gate=True)

        self.assertEqual(result.returncode, 0, msg=result.stderr)
        self.assertIn("[v1-gate] perf gate", result.stdout)
        self.assertIn("perf", result.log)


if __name__ == "__main__":
    unittest.main()
