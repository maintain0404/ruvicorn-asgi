use bytes::{Bytes, BytesMut};

use crate::handle::Handle;

use super::{
    bound::{RequestData, RequestHead},
    payload_handle::{LengthedPayload, PayloadType},
    state::State,
};

const MAX_HEADERS: usize = 96;

mod special_headers {
    pub const CONTENT_LENGTH: &str = "Content-Length";
    pub const TRANSFER_ENCODING: &str = "Transfer-Encoding";
    pub const CONNECTION: &str = "Connection";
}

#[derive(Debug)]
pub enum KeepAlive {
    KeepAlive,
    Close,
    None,
}

#[derive(Debug)]
pub struct ConnectionInfo {
    pub keep_alive: KeepAlive,
    pub payload_type: PayloadType,
}

#[derive(Debug)]
pub enum HeaderError {
    Duplicate(&'static str),
    InvalidValue(&'static str, &'static str),
}

#[derive(Debug)]
pub enum RequestError {
    InvalidRequest,
    InvalidHeader(HeaderError),
    PartialRequest,
}

impl From<HeaderError> for RequestError {
    fn from(error: HeaderError) -> Self {
        Self::InvalidHeader(error)
    }
}

#[derive(Debug)]
pub struct Request {}

fn cast_header(buffer: &BytesMut, header: &httparse::Header) -> (Bytes, Bytes) {
    let buf_ptr = buffer.as_ptr() as usize;
    dbg!(buf_ptr);

    let name_ptr = header.name.as_ptr() as usize;
    dbg!(name_ptr);
    let name = buffer
        .clone()
        .split_off(name_ptr - buf_ptr)
        .split_to(header.name.len())
        .freeze();

    let value_ptr = header.value.as_ptr() as usize;
    dbg!(value_ptr);
    let value = buffer
        .clone()
        .split_off(value_ptr - buf_ptr)
        .split_to(header.value.len())
        .freeze();

    return (name, value);
}

impl Request {
    fn iterate_headers(
        &self,
        buffer: &BytesMut,
        headers: &[httparse::Header],
    ) -> Result<(ConnectionInfo, Vec<(Bytes, Bytes)>), HeaderError> {
        let mut content_length: Option<u64> = None;

        let mut handled_te = false;
        let mut chunked = false;

        let mut keep_alive = KeepAlive::None;

        let mut headers_vec: Vec<(Bytes, Bytes)> = Vec::new();

        for header in headers.as_ref() {
            let name = header.name;
            let value = if let Ok(v) = std::str::from_utf8(header.value) {
                v
            } else {
                return Result::Err(HeaderError::InvalidValue(
                    ":value:",
                    "Not valid utf8 encoding.",
                ));
            }
            .trim();

            if special_headers::CONTENT_LENGTH.eq_ignore_ascii_case(name) {
                if chunked {
                    return Result::Err(HeaderError::InvalidValue(
                        special_headers::CONTENT_LENGTH,
                        "Content-Length header can'be with Transfer-Encoding: chunked header",
                    ));
                }

                if content_length.is_some() {
                    return Result::Err(HeaderError::Duplicate(special_headers::CONTENT_LENGTH));
                }

                if let Ok(len) = value.parse::<u64>() {
                    content_length = Some(len);
                } else {
                    return Result::Err(HeaderError::InvalidValue(
                        &special_headers::CONTENT_LENGTH,
                        "Content-Length header is not valid integer",
                    ));
                }
            } else if special_headers::TRANSFER_ENCODING.eq_ignore_ascii_case(name) {
                if handled_te {
                    return Result::Err(HeaderError::Duplicate(
                        &special_headers::TRANSFER_ENCODING,
                    ));
                } else {
                    handled_te = true;
                }

                for eachv in value.split(',').map(str::trim) {
                    if "chunked".eq_ignore_ascii_case(eachv) {
                        if content_length.is_some_and(|v| v > 0) {
                            return Result::Err(HeaderError::InvalidValue(
                                &special_headers::TRANSFER_ENCODING,
                                "Transfer-Encoding header can't be with Content-Length header",
                            ));
                        }
                        chunked = true;
                    } else if "identify".eq_ignore_ascii_case(eachv) {
                        // Pass
                    } else {
                        return Result::Err(HeaderError::InvalidValue(
                            &special_headers::TRANSFER_ENCODING,
                            "Transfer-Encoding header has invalild value",
                        ));
                    }
                }
            } else if special_headers::CONNECTION.eq_ignore_ascii_case(name) {
                keep_alive = if "keep-alive".eq_ignore_ascii_case(value) {
                    KeepAlive::KeepAlive
                } else if "close".eq_ignore_ascii_case(value) {
                    KeepAlive::Close
                } else {
                    KeepAlive::None
                }
            }

            let casted_header = cast_header(buffer, header);
            headers_vec.push(casted_header);
        }

        let payloadtype = if content_length.is_some_and(|v| v > 0) {
            PayloadType::Lenghthed(LengthedPayload {
                remaining: content_length.unwrap() as usize,
            })
        } else if chunked {
            PayloadType::Chunked
        } else {
            PayloadType::Lenghthed(LengthedPayload { remaining: 0 })
        };

        Ok((
            ConnectionInfo {
                keep_alive: keep_alive,
                payload_type: payloadtype,
            },
            headers_vec,
        ))
    }

    fn parse(
        &mut self,
        buffer: &mut BytesMut,
    ) -> Result<(RequestHead, ConnectionInfo), RequestError> {
        let mut headers = [httparse::EMPTY_HEADER; MAX_HEADERS];
        let mut req = httparse::Request::new(&mut headers);

        match req.parse(buffer.as_ref()) {
            Ok(status) => match status {
                httparse::Status::Complete(_) => {
                    let (info, headers) = self.iterate_headers(buffer, &req.headers)?;
                    return Ok((
                        RequestHead {
                            method: req.method.unwrap().to_owned(),
                            path: req.path.unwrap().to_owned(),
                            headers: headers,
                        },
                        info,
                    ));
                }
                httparse::Status::Partial => return Err(RequestError::PartialRequest),
            },
            Err(_) => return Err(RequestError::InvalidRequest),
        }
    }
}

impl Handle<RequestData, RequestHead, State, ConnectionInfo, RequestError> for Request {
    fn step(
        &mut self,
        buffer: &mut BytesMut,
        state: State,
        inbound: RequestData,
    ) -> Result<(RequestHead, State, ConnectionInfo), (RequestError, State)> {
        debug_assert!(matches!(state, State::Idle));
        dbg!(buffer.as_ptr() as usize);
        buffer.extend(&inbound.data);
        dbg!(buffer.as_ptr() as usize);
        match self.parse(buffer) {
            Ok((head, info)) => Ok((head, State::RequestHeadFinished, info)),
            Err(e) => Err((e, State::Closed)),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    macro_rules! data {
        ($code:expr) => {
            RequestData {
                data: Bytes::from_static($code),
            }
        };
    }

    #[test]
    fn test_partial_request() {
        let mut request = Request {};
        let mut buffer = BytesMut::new();
        let state = State::Idle;

        request
            .step(&mut buffer, state, data!(b"GET /"))
            .expect_err("Invalid request head");
    }

    #[test]
    fn test_get_request() {
        let mut request = Request {};
        let mut buffer = BytesMut::new();
        let state = State::Idle;

        let (head, state, conninfo) = request
            .step(
                &mut buffer,
                state,
                data!(b"GET /test HTTP/1.1\r\nHost:localhost\r\n\r\n"),
            )
            .unwrap();

        assert_eq!(head.method, "GET");
        assert_eq!(head.path, "/test");
        assert_eq!(head.headers.len(), 1);

        assert!(matches!(state, State::RequestHeadFinished));
        assert!(matches!(conninfo.keep_alive, KeepAlive::None));
        assert!(matches!(conninfo.payload_type, PayloadType::Lenghthed(len) if len.remaining == 0 ))
    }

    #[test]
    fn test_chunked() {
        let mut request = Request {};
        let mut buffer = BytesMut::new();
        let state = State::Idle;

        let (head, state, conninfo) = request
            .step(
                &mut buffer,
                state,
                data!(b"GET /test HTTP/1.1\r\nTransfer-Encoding:chunked\r\n\r\n"),
            )
            .unwrap();

        assert!(matches!(head, RequestHead { .. }));
        assert!(matches!(conninfo.payload_type, PayloadType::Chunked));
        assert!(matches!(state, State::RequestHeadFinished));
    }

    #[test]
    fn test_post_request() {
        let mut request = Request {};
        let mut buffer = BytesMut::new();
        let state = State::Idle;

        let (head, state, conninfo) = request
            .step(
                &mut buffer,
                state,
                data!(b"POST /test HTTP/1.1\r\nContent-Length:1\r\nHost:localhost\r\n\r\na"),
            )
            .unwrap();
        assert!(matches!(head, RequestHead { .. }));

        assert!(matches!(conninfo.payload_type, PayloadType::Lenghthed(len) if len.remaining == 1));
        assert!(matches!(state, State::RequestHeadFinished));
    }

    #[test]
    fn test_content_length_duplicate() {
        let mut request = Request {};
        let mut buffer = BytesMut::new();
        let state = State::Idle;

        let result = dbg!(request.step(
            &mut buffer,
            state,
            data!(b"GET /test HTTP/1.1\r\nContent-Length:1\r\nContent-Length:1\r\nHost:localhost\r\n\r\n"),
        ));

        assert!(result.is_err_and(|e| 
            matches!(e.0, RequestError::InvalidHeader(he) if matches!(he, HeaderError::Duplicate(name) if name == "Content-Length")) &&
            matches!(e.1, State::Closed)
        ));
    }

    #[test]
    fn test_content_length_invalid() {
        let mut request = Request {};
        let mut buffer = BytesMut::new();
        let state = State::Idle;

        assert!(request
            .step(
                &mut buffer,
                state,
                data!(b"GET /test HTTP/1.1\r\nContent-Length:s\r\nHost:localhost\r\n\r\n"),
            ).is_err_and(
                |e| matches!(e.0, 
                    RequestError::InvalidHeader(he) if 
                        matches!(he, HeaderError::InvalidValue(name, _) if name == "Content-Length")) &&
                    matches!(e.1, State::Closed)
            ));
    }

    #[test]
    fn test_tranfer_encoding_duplicate() {
        let mut request = Request {};
        let mut buffer = BytesMut::new();
        let state = State::Idle;

        let result = dbg!(request
        .step(
            &mut buffer,
            state,
            data!( b"GET /test HTTP/1.1\r\nTransfer-Encoding:chunked\r\nTransfer-Encoding:dup\r\nHost:localhost\r\n\r\n"),
        ));

        assert!(result.is_err_and(|e| matches!(e.0,
                    RequestError::InvalidHeader(he) if 
                        matches!(he, HeaderError::Duplicate(name) if name == "Transfer-Encoding"))
            && matches!(e.1, State::Closed)));
    }

    #[test]
    fn test_content_length_with_chunked() {
        let mut request = Request {};
        let mut buffer = BytesMut::new();
        let state = State::Idle;

        assert!(request
            .step(
                &mut buffer,
                state,
                data!(b"GET /test HTTP/1.1\r\nContent-Length:1\r\nTransfer-Encoding::chunked\r\nHost:localhost\r\n\r\n"),
            ).is_err_and(
                |e| matches!(e.0,
                    RequestError::InvalidHeader(he) if 
                        matches!(he, HeaderError::InvalidValue(name, _) if name == "Transfer-Encoding" || name == "Content-Length")) &&
                    matches!(e.1, State::Closed)    
                ));
    }

    #[test]
    fn test_keep_alive() {
        let mut request = Request {};
        let mut buffer = BytesMut::new();
        let state = State::Idle;

        let (_, state, conninfo) = request
            .step(
                &mut buffer,
                state,
                data!(b"GET /test HTTP/1.1\r\nConnection: keep-alive\r\nHost:localhost\r\n\r\n"),
            )
            .unwrap();

        assert!(matches!(conninfo.keep_alive, KeepAlive::KeepAlive));
        assert!(matches!(state, State::RequestHeadFinished));
    }

    #[test]
    fn test_close_connection() {
        let mut request = Request {};
        let mut buffer = BytesMut::new();
        let state = State::Idle;

        let (_, state, conninfo) = request
            .step(
                &mut buffer,
                state,
                data!(b"GET /test HTTP/1.1\r\nConnection: close\r\nHost:localhost\r\n\r\n"),
            )
            .unwrap();

        assert!(matches!(conninfo.keep_alive, KeepAlive::Close));
        assert!(matches!(state, State::RequestHeadFinished));
    }

    #[test]
    fn test_request_with_too_many_headers() {
        let mut request = Request {};
        let mut buffer = BytesMut::new();
        let state = State::Idle;
        let data = Bytes::from(
            [
                Vec::from(b"GET /test HTTP/1.1\r\n"),
                Vec::from(b"X:X\r\n".repeat(MAX_HEADERS + 1)),
                Vec::from(b"\r\n".to_owned()),
            ]
            .concat(),
        );

        assert!(request
            .step(&mut buffer, state, RequestData { data: data },)
            .is_err_and(
                |e| matches!(e.0, RequestError::InvalidRequest) && matches!(e.1, State::Closed)
            ));
    }
}
