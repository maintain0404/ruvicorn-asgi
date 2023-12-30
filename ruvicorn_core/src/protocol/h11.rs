use crate::{
    protocol::_base::{protocol_default_methods, Protocol},
    wrap_transport_method0, wrap_transport_method1,
    connection::h11::{Http11Connection, Event}
};
use pyo3::prelude::*;

#[pyclass(extends=Protocol, subclass)]
pub struct Http11Protocol {
    transport: Option<PyObject>,
    connection: Http11Connection,
}

impl Http11Protocol {
    wrap_transport_method1!(write, data, &[u8]);
    wrap_transport_method0!(close);

    fn handle_event(&mut self, py: Python<'_>, event: &Event) {
        match event {
            Event::PartialRequest => {},
            Event::RequestErr => self.send_400(py),
            Event::Request(_) => todo!(),
            Event::Data(_) => todo!(),
            Event::ChunkedData(_, _) => todo!(),
            Event::Eof => todo!(),
            Event::Close => self.close_connection(py),
        }
    }

    fn close_connection(&self, py: Python<'_>){
        self.close(py);
    }

    fn send_400(&self, py: Python<'_>) {
        self.write(py, b"HTTP/1.1 400 BAD_REQEUST\r\n\r\n");
        self.close(py);
    }
}

#[pymethods]
#[allow(unused_variables)]
impl Http11Protocol {
    protocol_default_methods!();

    fn data_received(&mut self, py: Python<'_>, data: &[u8]) -> PyResult<()> {
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
}
