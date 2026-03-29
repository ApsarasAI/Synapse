import os
import sys
import textwrap
from pathlib import Path


def _bootstrap_repo_sdk() -> None:
    if "__file__" in globals():
        repo_root = Path(__file__).resolve().parents[2]
    else:
        repo_root = Path.cwd()
    sdk_src = repo_root / "sdk" / "python" / "src"
    if sdk_src.is_dir():
        sys.path.insert(0, str(sdk_src))


_bootstrap_repo_sdk()

from synapse_sdk import SynapseClient, SynapseClientConfig


def build_review_script() -> str:
    return textwrap.dedent(
        """
        diff = '''
        diff --git a/app.py b/app.py
        index 1111111..2222222 100644
        --- a/app.py
        +++ b/app.py
        @@
        -def divide(a, b):
        -    return a / b
        +def divide(a, b):
        +    if b == 0:
        +        return 0
        +    return a / b
        '''

        findings = []
        if "return 0" in diff:
            findings.append(
                "Returning 0 on divide-by-zero hides an error condition and can corrupt callers."
            )

        if findings:
            print("PR review findings:")
            for item in findings:
                print(f"- {item}")
        else:
            print("No critical findings.")
        """
    ).strip() + "\n"


def main() -> None:
    client = SynapseClient(
        SynapseClientConfig(
            base_url=os.getenv("SYNAPSE_BASE_URL", "http://127.0.0.1:8080"),
            token=os.getenv("SYNAPSE_TOKEN"),
            tenant_id=os.getenv("SYNAPSE_TENANT_ID", "default"),
        )
    )
    response = client.execute(
        build_review_script(),
        request_id=os.getenv("SYNAPSE_REQUEST_ID", "pr-review-demo"),
    )
    print(response["stdout"], end="")


if __name__ == "__main__":
    main()
