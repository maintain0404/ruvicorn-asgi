use bytes::Bytes;
use pyo3::{types::{PyDict, PyString, PyType}, PyDowncastError, PyErr};

pub struct AsgiSendResponseStart {
    status: u32,
    headers: Vec<(Bytes, Bytes)>,
    trailers: bool
}

pub struct AsgiSendResponseBody {
    body: Bytes,
    more_body: bool
}


pub enum AsgiSend {
    Start(AsgiSendResponseStart),
    Body(AsgiSendResponseBody),
}


#[derive(Debug)]
pub struct InvalidAsgiSpec {}


impl From<PyDowncastError<'_>> for InvalidAsgiSpec {
    fn from(value: PyDowncastError<'_>) -> Self {
        todo!()
    }
}

impl From<PyErr> for InvalidAsgiSpec {
    fn from(value: PyErr) -> Self {
        todo!()
    }
}

impl TryFrom<&PyDict> for AsgiSendResponseStart {
    type Error = InvalidAsgiSpec;

    fn try_from(value: &PyDict) -> Result<Self, Self::Error> {
        todo!()
    }
}

impl TryFrom<&PyDict> for AsgiSendResponseBody {
    type Error = InvalidAsgiSpec;

    fn try_from(value: &PyDict) -> Result<Self, Self::Error> {
        todo!()
    }
}

impl TryFrom<&PyDict> for AsgiSend {
    type Error = InvalidAsgiSpec;

    fn try_from(value: &PyDict) -> Result<Self, InvalidAsgiSpec> {
        match value.get_item("type") {
            Some(type_) => {
                let typestr = type_.downcast::<PyString>()?.to_str()?;
                match typestr {
                    "http.response.start" => {
                        return Result::Ok(
                            Self::Start(AsgiSendResponseStart::try_from(value)?)
                        );
                    },
                    "http.response.body" => {
                        return Result::Ok(
                            Self::Body(AsgiSendResponseBody::try_from(value)?)
                        );
                    },
                    _ => {
                        return Result::Err(InvalidAsgiSpec{});
                    }
                }
            },
            None => {
                return Result::Err(InvalidAsgiSpec{})
            },
        }
    }
}


#[cfg(test)]
mod test {
    use std::convert;

    use bytes::Bytes;
    use pyo3::{types::PyDict, Python};

    use crate::event::send::AsgiSendResponseBody;

    use super::{AsgiSend, AsgiSendResponseStart};

    #[test]
    fn test_convert_http_response_start() {
        Python::with_gil(|py|{
            let result = py.eval("{'type':'http.response.body', 'status': 200, 'headers': [b'x-header-key':b'x-header-value'], 'trailers': False}", None, None)
            .map_err(|e| {
                e.print_and_set_sys_last_vars(py);
            }).unwrap();
            let res: &PyDict = result.extract().unwrap();
            let converted = AsgiSend::try_from(res).unwrap();
            let _headers: Vec<(Bytes, Bytes)> = vec![(
                Bytes::from_static(b"x-header-key"),
                Bytes::from_static(b"x-header-value")
            )];
            assert!(matches!(converted, AsgiSend::Start(AsgiSendResponseStart{
                status: 200,
                headers: _headers,
                trailers: false
            })))
    });
        
    }

    #[test]
    fn test_convert_http_response_body_without_headers() {
        Python::with_gil(|py|{
            let result = py.eval("{'type':'http.response.body', 'status': 200, 'trailers': False}", None, None)
            .map_err(|e| {
                e.print_and_set_sys_last_vars(py);
            }).unwrap();
            let res: &PyDict = result.extract().unwrap();
            let converted = AsgiSend::try_from(res).unwrap();
            let _headers: Vec<(Bytes, Bytes)> = Vec::new();
            assert!(matches!(converted, AsgiSend::Start(AsgiSendResponseStart{
                status: 200,
                headers: _headers,
                trailers: false
            })))
        })
    }

    #[test]
    fn test_convert_http_response_body_without_trailers() {
        Python::with_gil(|py|{
            let result = py.eval("{'type':'http.response.body', 'status': 200, 'headers': [b'x-header-key':b'x-header-value']}", None, None)
            .map_err(|e| {
                e.print_and_set_sys_last_vars(py);
            }).unwrap();
            let res: &PyDict = result.extract().unwrap();
            let converted = AsgiSend::try_from(res).unwrap();
            let _headers: Vec<(Bytes, Bytes)> = vec![(
                Bytes::from_static(b"x-header-key"),
                Bytes::from_static(b"x-header-value")
            )];
            assert!(matches!(converted, AsgiSend::Start(AsgiSendResponseStart{
                status: 200,
                headers: _headers,
                trailers: false
            })))
        })
    }

    #[test]
    fn test_convert_http_response_body_without_type() {
        Python::with_gil(|py|{
            let result = py.eval("{'status': 200, 'headers': [b'x-header-key':b'x-header-value'], 'trailers': False}", None, None)
            .map_err(|e| {
                e.print_and_set_sys_last_vars(py);
            }).unwrap();
            let res: &PyDict = result.extract().unwrap();
            assert!(AsgiSend::try_from(res).is_err());
        })
    }
}