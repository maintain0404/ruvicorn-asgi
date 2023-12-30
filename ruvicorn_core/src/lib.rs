use protocol::h11::Http11Protocol;
use pyo3::prelude::*;

mod protocol;
mod connection;

/// A Python module implemented in Rust.
#[pymodule]
fn ruvicorn_core(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<Http11Protocol>()?;
    Ok(())
}
