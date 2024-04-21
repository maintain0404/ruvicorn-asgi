use protocol::h11::Http11Protocol;
use pyo3::prelude::*;

mod conn;
mod connection;
mod errors;
mod event;
mod protocol;
mod types;

/// A Python module implemented in Rust.
#[pymodule]
#[pyo3(name = "_ruvicorn_core")]
fn _ruvicorn_core(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<Http11Protocol>()?;
    Ok(())
}
