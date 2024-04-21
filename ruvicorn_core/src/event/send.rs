use crate::errors::AsgiSpecError;
use crate::event::util;
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

#[cfg(test)]
mod test {
    use super::{ASGISendResponseBody, ASGISendResponseStart};
    use pyo3::{types::PyDict, Python};

    #[test]
    fn test_convert_http_response_start() {
        Python::with_gil(|py| {
            let result = py
                .eval(
                    r#"{
                    'type':'http.response.start',
                    'status': 200,
                    'headers': [(b'x-header-key', b'x-header-value')],
                    'trailers': False
                }"#,
                    None,
                    None,
                )
                .unwrap();
            let res: &PyDict = result.extract().unwrap();
            let headers: Vec<(&[u8], &[u8])> = vec![(b"x-header-key", b"x-header-value")];
            assert!(matches!(res.get_status(), Ok(200)));
            assert_eq!(res.get_headers(), headers);
            assert_eq!(res.get_trailers(), false);
        });
    }

    #[test]
    fn test_convert_http_response_start_with_emtpy_header() {
        Python::with_gil(|py| {
            let result = py
                .eval(
                    r#"{
                    'type':'http.response.start',
                    'status': 200,
                    'headers': [],
                    'trailers': False
                }"#,
                    None,
                    None,
                )
                .unwrap();
            let res: &PyDict = result.extract().unwrap();
            let headers: Vec<(&[u8], &[u8])> = vec![];
            assert!(matches!(res.get_status(), Ok(200)));
            assert_eq!(res.get_headers(), headers);
            assert_eq!(res.get_trailers(), false);
        });
    }

    #[test]
    fn test_convert_http_response_start_without_trailers() {
        Python::with_gil(|py| {
            let result = py
                .eval(
                    r#"{
                    'type':'http.response.start',
                    'status': 200,
                    'headers': [(b'x-header-key', b'x-header-value')],
                }"#,
                    None,
                    None,
                )
                .unwrap();
            let res: &PyDict = result.extract().unwrap();
            let headers: Vec<(&[u8], &[u8])> = vec![(b"x-header-key", b"x-header-value")];
            assert!(matches!(res.get_status(), Ok(200)));
            assert_eq!(res.get_headers(), headers);
            assert_eq!(res.get_trailers(), false);
        });
    }

    #[test]
    fn test_convert_http_response_start_without_headers() {
        Python::with_gil(|py| {
            let result = py
                .eval(
                    r#"{
                    'type':'http.response.start',
                    'status': 200,
                    'trailers': False
                }"#,
                    None,
                    None,
                )
                .unwrap();
            let res: &PyDict = result.extract().unwrap();
            let headers: Vec<(&[u8], &[u8])> = vec![];
            assert!(matches!(res.get_status(), Ok(200)));
            assert_eq!(res.get_headers(), headers);
            assert_eq!(res.get_trailers(), false);
        });
    }

    #[test]
    fn test_convert_http_response_body() {
        Python::with_gil(|py| {
            let result = py
                .eval(
                    r#"{
                    'type':'http.response.body',
                    'body': b'asdf',
                    'more_body': True,
                }"#,
                    None,
                    None,
                )
                .unwrap();
            let res: &PyDict = result.extract().unwrap();
            assert_eq!(res.get_body(), b"asdf");
            assert_eq!(res.get_more_body(), true);
        });
    }

    #[test]
    fn test_convert_http_response_body_without_body() {
        Python::with_gil(|py| {
            let result = py
                .eval(
                    r#"{
                    'type':'http.response.body',
                    'more_body': True,
                }"#,
                    None,
                    None,
                )
                .unwrap();
            let res: &PyDict = result.extract().unwrap();
            assert_eq!(res.get_body(), b"");
            assert_eq!(res.get_more_body(), true);
        });
    }

    #[test]
    fn test_convert_http_response_body_without_more_body() {
        Python::with_gil(|py| {
            let result = py
                .eval(
                    r#"{
                    'type':'http.response.body',
                    'body': b'asdf'
                }"#,
                    None,
                    None,
                )
                .unwrap();
            let res: &PyDict = result.extract().unwrap();
            assert_eq!(res.get_body(), b"asdf");
            assert_eq!(res.get_more_body(), false);
        });
    }
}
