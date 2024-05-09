
use pyo3::prelude::*;

mod conn;
mod errors;
mod event;
mod types;

/// A Python module implemented in Rust.
#[pymodule]
#[pyo3(name = "_ruvicorn_core")]
fn _ruvicorn_core(_py: Python, m: &PyModule) -> PyResult<()> {
    Ok(())
}
