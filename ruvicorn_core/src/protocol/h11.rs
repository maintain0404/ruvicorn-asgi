use crate::{
    connection::h11::{Event, Http11Connection},
    wrap_transport_method0, wrap_transport_method1,
};
use pyo3::prelude::*;

#[pyclass]
pub struct Http11Protocol {
    transport: Option<PyObject>,
    connection: Http11Connection,
}

impl Http11Protocol {
    wrap_transport_method1!(write, data, &[u8]);
    wrap_transport_method0!(close);

    fn handle_event(&mut self, py: Python<'_>, event: &Event) {
        match event {
            Event::PartialRequest => {}
            Event::RequestErr => self.send_400(py),
            Event::Request => {
                let next = self.connection.next();
                self.handle_event(py, &next);
            }
            Event::Data(data) => {
                let data_u8 = data.as_ref();
                println!("{}", std::str::from_utf8(data_u8).unwrap());
                // TODO: Run ASGI app here.
            }
            Event::ChunkedData(_, _) => todo!(),
            Event::Eof => {
                let next = self.connection.next();
                self.handle_event(py, &next);
            }
            Event::Close => self.close_connection(py),
            Event::Idle => self.preserve_timeout(),
        }
    }

    fn preserve_timeout(&self) {}

    fn reset_timeout(&mut self) {}

    fn close_connection(&self, py: Python<'_>) {
        self.close(py);
    }

    fn send_400(&self, py: Python<'_>) {
        self.write(py, b"HTTP/1.1 400 BAD_REQUEST\r\n\r\n");
        self.close(py);
    }
}

#[pymethods]
#[allow(unused_variables)]
impl Http11Protocol {
    #[new]
    fn new() -> Self {
        Self {
            transport: None,
            connection: Http11Connection::new(),
        }
    }

    fn connection_made(&mut self, transport: PyObject) -> PyResult<()> {
        self.transport = Some(transport);
        Ok(())
    }

    fn data_received(&mut self, py: Python<'_>, data: &[u8]) -> PyResult<()> {
        self.reset_timeout();
        self.connection.feed(data);
        let event = self.connection.next();
        self.handle_event(py, &event);
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
