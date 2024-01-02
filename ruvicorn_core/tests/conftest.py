from maturin import import_hook

# install the import hook with default settings
import_hook.install()
# or you can specify bindings
import_hook.install(bindings="pyo3")
# and build in release mode instead of the default debug mode
import_hook.install(release=True)
