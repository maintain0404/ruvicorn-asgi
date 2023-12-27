use pyo3::prelude::*;


#[pyclass(subclass)] 
pub struct Protocol {
    transport: Option<PyObject>
}

#[macro_export]
macro_rules! wrap_transport_method0 {
    ($method_name:ident) => {
        fn $method_name(&self, py: Python<'_>) {
            self.transport.as_ref().unwrap().call_method0(py, stringify!($method_name)).unwrap();
        }
    }
}
pub use wrap_transport_method0;

#[macro_export]
macro_rules! wrap_transport_method1 {
    ($method_name:ident, $param_name:ident, $param_type:ty) => {
        fn $method_name (&self, py: Python<'_>, $param_name: $param_type) {
            self.transport.as_ref().unwrap().call_method1(py, stringify!($method_name), ($param_name,)).unwrap();
        }
    }
}
pub use wrap_transport_method1;

#[macro_export]
macro_rules! protocol_default_methods {
    () => {
        #[allow(dead_code)]
        fn connection_made(&mut self, transport: PyObject) -> PyResult<()> {
            self.transport = Some(transport);
            Ok(())
        }

        fn __traverse__(&self, visit: pyo3::PyVisit<'_>) -> Result<(), pyo3::PyTraverseError> {
            if let Some(obj) = &self.transport {
                visit.call(obj)?
            }
            Ok(())
        }
    
        fn __clear__(&mut self) {
            self.transport = None;
        }
    }
}
pub use protocol_default_methods;

impl Protocol {
    wrap_transport_method1!(write, data, &[u8]);
    wrap_transport_method0!(close);
}


#[pymethods]
impl Protocol {
    #[new]
    unsafe fn new() -> Self {
        Self { transport: None }
    }

    fn connection_made(&mut self, transport: PyObject) -> PyResult<()> {
        self.transport = Some(transport);
        Ok(())
    }

    #[pyo3(signature = (data))]
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

    #[allow(unused_variables)]
    fn connection_lost(self_: PyRef<'_, Self>, err: &PyAny) -> PyResult<()>{
        println!("Closed!");
        Ok(())
    }
}

