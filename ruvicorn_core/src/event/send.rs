use crate::event::util;
use crate::errors::AsgiSpecError;
use crate::types::PyHeader;
use pyo3::types::PyDict;

pub trait ASGISendResponseStart: Sized {
    fn get_status(&self) -> Result<usize, AsgiSpecError>;

    fn get_headers(&self) -> Vec<PyHeader>;

    fn get_trailers(&self) -> bool;
}

impl ASGISendResponseStart for &PyDict {
    fn get_status(&self) -> Result<usize, AsgiSpecError> {
        util::get_item_with_casting(self, "status")
    }

    fn get_headers(&self) -> Vec<PyHeader> {
        util::get_item_with_default(self, "headers", Vec::new())
    }

    fn get_trailers(&self) -> bool {
        util::get_item_with_default(self, "trailers", false)
    }
}

pub trait ASGISendResponseBody: Sized {
    fn get_body(&self) -> &[u8];

    fn get_more_body(&self) -> bool;
}

impl ASGISendResponseBody for &PyDict {
    fn get_body(&self) -> &[u8] {
        util::get_item_with_default(self, "body", b"")
    }

    fn get_more_body(&self) -> bool {
        util::get_item_with_default(self, "more_body", false)
    }
}

mod test {}
