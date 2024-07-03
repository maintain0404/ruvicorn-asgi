use crate::handle::Handle;
use bytes::{BufMut, BytesMut};
use http::{status::InvalidStatusCode, StatusCode};

use super::{
    bound::{ResponseData, ResponseStart},
    state::State,
};

#[derive(Debug)]
enum ResponseError {
    InvalidStatusCode(InvalidStatusCode),
}

impl From<InvalidStatusCode> for ResponseError {
    fn from(value: InvalidStatusCode) -> Self {
        Self::InvalidStatusCode(value)
    }
}

struct ResponseHead {}

impl Handle<ResponseStart, ResponseData, State, (), ResponseError> for ResponseHead {
    fn step(
        &mut self,
        buffer: &mut BytesMut,
        state: State,
        inbound: ResponseStart,
    ) -> Result<(ResponseData, State, ()), (ResponseError, State)> {
        debug_assert!(matches!(state, State::RequestBodyFinished));

        let status_code = match StatusCode::from_u16(inbound.status) {
            Ok(code) => code,
            Err(e) => return Err((ResponseError::from(e), State::Closed)),
        };
        buffer.put_slice(b"HTTP/1.1 ");
        buffer.put_slice(status_code.as_str().as_bytes());
        buffer.put_slice(b" ");
        buffer.put_slice(
            status_code
                .canonical_reason()
                .map_or_else(|| "", |s| s)
                .as_bytes(),
        );
        buffer.put_slice(b"\r\n");

        for (name, value) in inbound.headers {
            buffer.put_slice(name.as_ref());
            buffer.put_slice(b": ");
            buffer.put_slice(value.as_ref());
            buffer.put_slice(b"\r\n");
        }
        buffer.put_slice(b"\r\n");

        let res_bytes = buffer.clone().freeze();
        buffer.clear();

        return Ok((
            ResponseData { data: res_bytes },
            State::ResponseHeadFinished,
            (),
        ));
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use bytes::Bytes;

    macro_rules! header {
        ($name:expr, $value:expr) => {
            (Bytes::from_static($name), Bytes::from_static($value))
        };
    }

    #[test]
    fn test_response_head() {
        let mut response = ResponseHead {};
        let mut buffer = BytesMut::new();
        let state = State::RequestBodyFinished;
        let inbound = ResponseStart {
            status: 200,
            headers: vec![header!(b"Name", b"Value"), header!(b"Name2", b"Value2")],
        };

        let (data, state, _) = dbg!(response.step(&mut buffer, state, inbound)).unwrap();

        assert!(matches!(state, State::ResponseHeadFinished));
        assert_eq!(
            data.data,
            Bytes::from_static(b"HTTP/1.1 200 OK\r\nName: Value\r\nName2: Value2\r\n\r\n")
        );
    }

    #[test]
    fn test_response_head_invalid_status_code() {
        let mut response = ResponseHead {};
        let mut buffer = BytesMut::new();
        let state = State::RequestBodyFinished;
        let inbound = ResponseStart {
            status: 1001,
            headers: vec![header!(b"Name", b"Value"), header!(b"Name2", b"Value2")],
        };

        let result = dbg!(response.step(&mut buffer, state, inbound));

        assert!(matches!(result, Err(e) if 
            matches!(e.0, ResponseError::InvalidStatusCode(_)) && matches!(e.1, State::Closed)))
    }

    #[test]
    fn test_response_head_with_no_canonical_reason() {
        let mut response = ResponseHead {};
        let mut buffer = BytesMut::new();
        let state = State::RequestBodyFinished;
        let inbound = ResponseStart {
            status: 999,
            headers: vec![header!(b"Name", b"Value"), header!(b"Name2", b"Value2")],
        };

        let result = dbg!(response.step(&mut buffer, state, inbound));
        let (data, state, _) = result.unwrap();

        assert!(matches!(state, State::ResponseHeadFinished));
        assert_eq!(
            data.data,
            Bytes::from_static(b"HTTP/1.1 999 \r\nName: Value\r\nName2: Value2\r\n\r\n")
        );
    }
}
