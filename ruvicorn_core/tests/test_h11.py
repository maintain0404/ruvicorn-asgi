import asyncio as aio
from typing import TypeAlias

import pytest

from ruvicorn_core import Http11Protocol

Server: TypeAlias = tuple[str, int]


@pytest.fixture
async def server(unused_tcp_port: int) -> Server:
    srv = await aio.get_running_loop().create_server(
        lambda: Http11Protocol(), "127.0.0.1", unused_tcp_port
    )

    async with srv:
        await srv.start_serving()
        yield ("localhost", unused_tcp_port)


async def test_invalid_request(server: Server):
    reader, writer = await aio.open_connection(server[0], server[1])

    print("Try writting request...")
    writer.write(b"GET 400\r\n")
    await aio.wait_for(writer.drain(), 1)
    print("Write finished")

    print("Try reading response...")
    res = await aio.wait_for(reader.read(100), 1)
    print("Read finished")

    # Check connection closed by peer.
    await aio.wait_for(reader.read(), 1)

    assert res == b"HTTP/1.1 400 BAD_REQUEST\r\n\r\n"
