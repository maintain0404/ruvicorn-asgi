use bytes::{BufMut, Bytes, BytesMut};

use crate::types::{PyHeader, RsHeader};

use super::payload::{Payload as _, PayloadStepResult, PayloadType};

const MAX_HEADERS: usize = 16;

macro_rules! test_trace {
    ($code:expr, $msg:expr) => {
        #[cfg(test)]
        {
            println!("{}: {}", $code, $msg);
        }
    };
}

enum State {
    // Ready to get request.
    Idle,
    // Request parse finished. Ready for get body.
    RequestHeadFinished,
    // Get all requet body data. Ready to response.
    RequestBodyFinished,
    // Response Head parse finished. Ready for send body.
    ResponseHeadFinished,
    // Connection closed by error or finished all request/response cycle.
    Closed,
}

#[derive(Debug)]
#[allow(dead_code)]
enum Input<'t> {
    // Feed data
    RequestData(&'t [u8]),
    // Notice physical connection is closed.
    Disconnect,

    ResponseStart {
        status: usize,
        headers: Vec<PyHeader<'t>>,
    },
    ResponseBody {
        body: &'t [u8],
        more_body: bool,
    },
}

#[derive(Debug)]
#[allow(dead_code)]
enum Output {
    // Request is not finished or invalid.

    // Need more data to finish request.
    PartialRequest,
    // Request do not object HTTP spec.
    RequestErr,

    // Request is finished.

    // Request finished with Content-Length header.
    RequestHead {
        method: String,
        path: String,
        headers: Vec<RsHeader>,
    },
    RequestBody(Bytes, bool),

    // Invalid response spec
    ReseponseErr,

    // Need more data to finish response
    PartialResponse,

    ResponseStart(Bytes),
    ResponseBody(Bytes),
}

#[allow(dead_code)]
impl Output {
    fn is_request_head_finished(&self) -> bool {
        return match self {
            Self::PartialRequest => false,
            Self::RequestErr => false,
            _ => true,
        };
    }

    #[allow(dead_code)]
    fn is_error(&self) -> bool {
        return match self {
            Self::RequestErr => true,
            _ => false,
        };
    }
}

#[derive(Debug)]
pub enum KeepAlive {
    KeepAlive,
    Close,
    None,
}

impl KeepAlive {
    fn should_keep_alive(&self) -> bool {
        match self {
            Self::KeepAlive => true,
            _ => false,
        }
    }
}

struct Http11Connection {
    req_buffer: BytesMut,
    res_buffer: BytesMut,
    state: State,
    offset: usize,
    keep_alive: KeepAlive,
    payload: PayloadType,
}

mod special_headers {
    pub const CONTENT_LENGTH: &str = "Content-Length";
    pub const TRANSFER_ENCODING: &str = "Transfer-Encoding";
    pub const CONNECTION: &str = "Connection";
    #[allow(dead_code)]
    pub const UPGRADE: &str = "Upgrade";
}

fn cast_header_to_rs_header(buffer: &BytesMut, header: &httparse::Header) -> RsHeader {
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

fn cast_headers_to_rs_headers(buffer: &BytesMut, headers: &[httparse::Header]) -> Vec<RsHeader> {
    let mut ret: Vec<RsHeader> = Vec::new();
    ret.reserve(headers.len());

    for header in headers {
        if header.name.is_empty() {
            break;
        }
        ret.push(cast_header_to_rs_header(buffer, header))
    }

    return ret;
}

#[allow(dead_code)]
impl Http11Connection {
    fn new() -> Self {
        Self {
            req_buffer: BytesMut::new(),
            res_buffer: BytesMut::new(),
            state: State::Idle,
            offset: 0,
            keep_alive: KeepAlive::None,
            payload: PayloadType::new_none(),
        }
    }

    fn _iterate_headers(
        &self,
        headers: &[httparse::Header],
    ) -> Result<(PayloadType, KeepAlive), ()> {
        let mut content_length: u64 = 0;

        let mut handled_te = false;
        let mut chunked = false;

        let mut keep_alive = KeepAlive::None;

        for header in headers.as_ref() {
            let name = header.name;

            let value = if let Ok(v) = std::str::from_utf8(header.value) {
                v
            } else {
                return Result::Err(());
            }
            .trim();

            if special_headers::CONTENT_LENGTH.eq_ignore_ascii_case(name) {
                // Content Length Header duplicate.
                if content_length != 0 || chunked {
                    test_trace!(
                        "h11.header.content-length-duplicate",
                        "Content-Length header duplicate or Transfer-Encoding is already set chunked."
                    );
                    return Result::Err(());
                }

                if let Ok(len) = value.parse::<u64>() {
                    content_length = len;
                } else {
                    test_trace!(
                        "h11.header.invalid-content-length",
                        "Invalid Content-Length Header"
                    );
                    return Result::Err(());
                }
            } else if special_headers::TRANSFER_ENCODING.eq_ignore_ascii_case(name) {
                if handled_te {
                    test_trace!(
                        "h11.header.transfer-encoding-duplicate",
                        "Tranfer-Encoding header is duplicated."
                    );
                    return Result::Err(());
                } else {
                    handled_te = true;
                }

                for eachv in value.split(',').map(str::trim) {
                    if "chunked".eq_ignore_ascii_case(eachv) {
                        if content_length > 0 {
                            test_trace!(
                                "h11.header.content-length-with-chunked",
                                "Content-Length headerris already set."
                            );
                            return Result::Err(());
                        }
                        chunked = true;
                    } else if "identify".eq_ignore_ascii_case(eachv) {
                        // Pass
                    } else {
                        return Result::Err(());
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
        }

        if content_length > 0 {
            return Ok((
                PayloadType::new_lengthed(content_length as usize),
                keep_alive,
            ));
        } else if chunked {
            return Ok((PayloadType::new_chunked(), keep_alive));
        } else {
            return Ok((PayloadType::new_none(), keep_alive));
        }
    }

    fn parse_request_head(&mut self) -> Output {
        let mut headers = [httparse::EMPTY_HEADER; MAX_HEADERS];
        let mut req = httparse::Request::new(&mut headers);

        match req.parse(self.req_buffer.as_ref()) {
            Ok(status) => match status {
                httparse::Status::Complete(offset) => {
                    self.offset = offset;
                    self.state = State::RequestHeadFinished;
                    if let Ok((payload, keep_alive)) = self._iterate_headers(&req.headers) {
                        self.payload = payload;
                        self.keep_alive = keep_alive;

                        Output::RequestHead {
                            method: req.method.unwrap().to_owned(),
                            path: req.path.unwrap().to_owned(),
                            headers: cast_headers_to_rs_headers(&self.req_buffer, &headers),
                        }
                    } else {
                        self.state = State::Closed;
                        Output::RequestErr
                    }
                }
                httparse::Status::Partial => Output::PartialRequest,
            },
            Err(e) => {
                println!("Parsing failed with \"{}\"", e);
                self.state = State::Closed;
                Output::RequestErr
            }
        }
    }

    fn parse_body(&mut self) -> Output {
        match self.payload.step(&mut self.req_buffer, self.offset) {
            PayloadStepResult::Partial(body, offset) => {
                self.offset = offset;
                Output::RequestBody(body, false)
            }
            PayloadStepResult::Finished(body, offset) => {
                self.offset = offset;
                self.state = State::RequestBodyFinished;
                Output::RequestBody(body, true)
            }
            PayloadStepResult::Err => Output::RequestErr,
        }
    }

    fn _feed(&mut self, data: &[u8]) -> Output {
        self.req_buffer.extend(data);
        match self.state {
            State::Idle => self.parse_request_head(),
            State::RequestHeadFinished => self.parse_body(),
            _ => todo!()
        }
    }

    fn start_response(&mut self, status: usize, headers: Vec<PyHeader>) -> Output {
        // Write response
        self.res_buffer.put_slice(b"HTTP/1.1 ");
        let status_code = status.to_string();
        self.res_buffer.put_slice(status_code.as_bytes());
        self.res_buffer.put_slice(b"\r\n");

        for (name, value) in headers {
            self.res_buffer.put_slice(name);
            self.res_buffer.put_slice(b": ");
            self.res_buffer.put_slice(value);
            self.res_buffer.put_slice(b"\r\n");
        }
        self.res_buffer.put_slice(b"\r\n");

        let res_bytes = self.res_buffer.clone().freeze();
        self.state = State::ResponseHeadFinished;
        self.res_buffer.clear();
        return Output::ResponseStart(res_bytes);
    }
    

    fn step(&mut self, input: Input) -> Output {
        match input {
            Input::RequestData(data) => self._feed(data),
            Input::Disconnect => todo!(),
            Input::ResponseStart { status, headers } => self.start_response(status, headers),
            Input::ResponseBody { body, more_body } => todo!(),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_partial_request() {
        let mut conn = Http11Connection::new();

        let output = dbg!(conn.step(Input::RequestData(b"GET /")));
        assert!(matches!(output, Output::PartialRequest))
    }

    #[test]
    fn test_get_request() {
        let mut conn = Http11Connection::new();

        let output = dbg!(conn.step(Input::RequestData(
            b"GET /test HTTP/1.1\r\nHost:localhost\r\n\r\n"
        )));
        assert!(matches!(output, Output::RequestHead { .. }));
        assert!(matches!(conn.payload, PayloadType::None(_)));
        assert!(matches!(conn.state, State::RequestHeadFinished));
    }

    #[test]
    fn test_chunked() {
        let mut conn = Http11Connection::new();

        let output = dbg!(conn.step(Input::RequestData(
            b"GET /test HTTP/1.1\r\nTransfer-Encoding:chunked\r\nHost:localhost\r\n\r\n"
        )));
        assert!(matches!(output, Output::RequestHead { .. }));
        assert!(matches!(conn.payload, PayloadType::ChunkedPayload(_)));
        assert!(matches!(conn.state, State::RequestHeadFinished));
    }

    #[test]
    fn test_post_request() {
        let mut conn = Http11Connection::new();

        let output = dbg!(conn.step(Input::RequestData(
            b"POST /test HTTP/1.1\r\nContent-Length:1\r\nHost:localhost\r\n\r\na"
        )));
        assert!(matches!(output, Output::RequestHead { .. }));

        if let PayloadType::LengthedPayload(p) = conn.payload {
            assert_eq!(p.to_consume, 1)
        }
        assert!(matches!(conn.state, State::RequestHeadFinished));
    }

    #[test]
    fn test_content_length_duplicate() {
        let mut conn = Http11Connection::new();

        let output = dbg!(conn.step(Input::RequestData(
            b"GET /test HTTP/1.1\r\nContent-Length:1\r\nContent-Length:1\r\nHost:localhost\r\n\r\n"
        )));
        assert!(matches!(output, Output::RequestErr));
        assert!(matches!(conn.state, State::Closed));
    }

    #[test]
    fn test_content_length_invalid() {
        let mut conn = Http11Connection::new();

        let output = dbg!(conn.step(Input::RequestData(
            b"GET /test HTTP/1.1\r\nContent-Length:s\r\nHost:localhost\r\n\r\n"
        )));
        assert!(matches!(output, Output::RequestErr));
        assert!(matches!(conn.state, State::Closed));
    }

    #[test]
    fn test_tranfer_encoding_duplicate() {
        let mut conn = Http11Connection::new();

        let output = dbg!(conn.step(Input::RequestData(
            b"GET /test HTTP/1.1\r\nTransfer-Encoding:chunked\r\nTransfer-Encoding:dup\r\nHost:localhost\r\n\r\n",
        )));
        assert!(matches!(output, Output::RequestErr));
        assert!(matches!(conn.state, State::Closed));
    }

    #[test]
    fn test_content_length_with_chunked() {
        let mut conn = Http11Connection::new();

        let output = dbg!(conn.step(Input::RequestData(
            b"GET /test HTTP/1.1\r\nContent-Length:1\r\nTransfer-Encoding::chunked\r\nHost:localhost\r\n\r\n"
        )));
        assert!(matches!(output, Output::RequestErr));
        assert!(matches!(conn.state, State::Closed));
    }

    #[test]
    fn test_keep_alive() {
        let mut conn = Http11Connection::new();

        let output = dbg!(conn.step(Input::RequestData(
            b"GET /test HTTP/1.1\r\nConnection: keep-alive\r\nHost:localhost\r\n\r\n"
        )));
        assert!(matches!(output, Output::RequestHead { .. }));
        assert!(matches!(conn.keep_alive, KeepAlive::KeepAlive));
        assert!(matches!(conn.state, State::RequestHeadFinished));
    }

    #[test]
    fn test_close_connection() {
        let mut conn = Http11Connection::new();

        let output = dbg!(conn.step(Input::RequestData(
            b"GET /test HTTP/1.1\r\nConnection: close\r\nHost:localhost\r\n\r\n"
        )));
        assert!(matches!(output, Output::RequestHead { .. }));
        assert!(matches!(conn.keep_alive, KeepAlive::Close));
        assert!(matches!(conn.state, State::RequestHeadFinished));
    }

    #[test]
    fn test_request_with_too_many_headers() {
        let mut conn = Http11Connection::new();
        let data = [
            Vec::from(b"GET /test HTTP/1.1\r\n"),
            Vec::from(b"X:X\r\n".repeat(MAX_HEADERS + 1)),
            Vec::from(b"\r\n".to_owned()),
        ]
        .concat();

        let output = dbg!(conn.step(Input::RequestData(data.as_ref())));
        assert!(matches!(output, Output::RequestErr));
        assert!(matches!(conn.state, State::Closed));
    }

    #[test]
    fn test_response_head() {
        let mut conn = Http11Connection::new();
        conn.state = State::RequestBodyFinished;
        
        let output = conn.step(Input::ResponseStart { status: 200, headers: vec![(b"Name", b"Value"), (b"Name2", b"Value2")] });
        assert!(matches!(conn.state, State::ResponseHeadFinished));
        assert_eq!(conn.res_buffer.len(), 0);
        match output {
            Output::ResponseStart(data) => {
                assert_eq!(data.clone(), Bytes::from_static(b"HTTP/1.1 200\r\nName: Value\r\nName2: Value2\r\n\r\n"));
            },
            _ => assert_eq!(1, 0)
        }
    }

}
