use bytes::Bytes;
use pyo3::types::PyDict;

pub struct AsgiReceiveRequest {
    body: Bytes,
    more_body: bool
}

pub enum AsgiReceive{
    Request(AsgiReceiveRequest),
    Disconnect,
}

