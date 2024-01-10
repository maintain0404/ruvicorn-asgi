from maturin import import_hook

# install the import hook with default settings
import_hook.install()
# or you can specify bindings
import_hook.install(bindings="pyo3")
# and build in release mode instead of the default debug mode
import_hook.install(release=True)


from ruvicorn_core import Http11Protocol
import asyncio as aio


async def main():
    loop = aio.get_event_loop()

    srv = await loop.create_server(
        lambda: Http11Protocol(),
        "127.0.0.1",
        8888
    )

    async with srv:
        await srv.serve_forever()

aio.run(main(), debug=True)
