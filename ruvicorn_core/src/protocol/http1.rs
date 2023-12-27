use crate::{
    protocol::_base::{protocol_default_methods, Protocol},
    wrap_transport_method0, wrap_transport_method1,
};
use pyo3::prelude::*;

#[pyclass(extends=Protocol)]
struct Http1Protocol {
    transport: Option<PyObject>,
}

impl Http1Protocol {
    wrap_transport_method1!(write, data, &[u8]);
    wrap_transport_method0!(close);
}

#[pymethods]
#[allow(unused_variables)]
impl Http1Protocol {
    protocol_default_methods!();

    fn data_received(&self, py: Python<'_>, data: &[u8]) -> PyResult<()> {
        println!("Got data {}", data.into_py(py));
        self.write(py, data);
        self.close(py);
        Ok(())
    }

    fn eof_received(&self) -> PyResult<()> {
        println!("EOF");
        Ok(())
    }

    fn connection_lost(self_: PyRef<'_, Self>, err: &PyAny) -> PyResult<()> {
        println!("Closed!");
        Ok(())
    }
}
