import asyncio

from synapse_sdk import SynapseClient, SynapseClientConfig


async def main() -> None:
    client = SynapseClient(
        SynapseClientConfig(
            base_url="http://127.0.0.1:8080",
            token="replace-me",
            tenant_id="default",
        )
    )

    async for event in client.execute_stream("print('stream sdk')\n"):
        print(event)


asyncio.run(main())
