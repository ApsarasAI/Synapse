from synapse_sdk import SynapseClient, SynapseClientConfig


client = SynapseClient(
    SynapseClientConfig(
        base_url="http://127.0.0.1:8080",
        token="replace-me",
        tenant_id="default",
    )
)

result = client.execute("print('hello from sdk')\n")
print(result["stdout"], end="")
