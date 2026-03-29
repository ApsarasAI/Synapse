# synapse-sdk

Minimal Python SDK for the Synapse v1 HTTP and websocket APIs.

## Covered surface

- `execute`
- `execute_stream`
- bearer token header injection
- tenant header injection
- basic error mapping

## Install

```bash
pip install -e sdk/python
```

## Example

```python
from synapse_sdk import SynapseClient, SynapseClientConfig

client = SynapseClient(
    SynapseClientConfig(
        base_url="http://127.0.0.1:8080",
        token="dev-token",
        tenant_id="default",
    )
)

response = client.execute("print('hello')\n", request_id="sdk-readme-demo")
print(response["stdout"])
```

See `examples/pr-review-agent/` for the standard demo path.
That demo bootstraps `sdk/python/src` automatically when run from the repo checkout, so a separate editable install is not required for local smoke runs.
