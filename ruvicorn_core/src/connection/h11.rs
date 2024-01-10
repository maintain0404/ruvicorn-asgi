use bytes::{Bytes, BytesMut};

macro_rules! test_trace {
    ($code:expr, $msg:expr) => {
        #[cfg(test)]
        {
            println!("{}: {}", $code, $msg);
        }
    };
}

#[derive(Debug)]
pub enum KeepAlive {
    KeepAlive,
    Close,
    None,
}

#[derive(Debug)]
pub enum Payload {
    WebSocketUpgrade,
    ChunkedPayload,
    Payload(u64),
    None,
}

#[derive(Debug)]
pub enum Event {
    PartialRequest,
    RequestErr,
    Request,
    Data(Bytes),
    ChunkedData(Bytes, bool),
    Eof,
    Idle,
    Close,
}

enum State {
    Idle,
    RequestFinished,
    Eof,
    Closed,
}

trait PayloadDecoder {
    fn feed(&mut self, data: &[u8]);

    fn next(&mut self) -> Event;
}

pub struct Http11Connection {
    buffer: BytesMut,
    state: State,
    offset: usize,
    keep_alive: KeepAlive,
    payload: Payload,
}

mod special_headers {
    pub const CONTENT_LENGTH: &str = "Content-Length";
    pub const TRANSFER_ENCODING: &str = "Transfer-Encoding";
    pub const CONNECTION: &str = "Connection";
    pub const UPGRADE: &str = "Upgrade";
}

impl Http11Connection {
    pub fn new() -> Self {
        Self {
            buffer: BytesMut::new(),
            state: State::Idle,
            offset: 0,
            keep_alive: KeepAlive::None,
            payload: Payload::None,
        }
    }

    fn _iterate_headers(&self, headers: &[httparse::Header]) -> Result<(Payload, KeepAlive), ()> {
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
            return Ok((Payload::Payload(content_length), keep_alive));
        } else if chunked {
            return Ok((Payload::ChunkedPayload, keep_alive));
        } else {
            return Ok((Payload::None, keep_alive));
        }
    }

    fn next_data(&mut self) -> (Bytes, bool) {
        match self.payload {
            Payload::WebSocketUpgrade => todo!(),
            Payload::ChunkedPayload => todo!(),
            Payload::Payload(length) => {
                self.state = State::Eof;
                (
                    self.buffer
                        .to_owned()
                        .freeze()
                        .split_off(self.offset)
                        .split_to(length as usize),
                    false,
                )
            }
            Payload::None => todo!(),
        }
    }

    pub fn feed(&mut self, data: &[u8]) {
        self.buffer.extend(data);
    }

    pub fn next(&mut self) -> Event {
        match self.state {
            State::Idle => {
                let mut headers = [httparse::EMPTY_HEADER; 16];
                let mut req = httparse::Request::new(&mut headers);
                match req.parse(self.buffer.as_ref()) {
                    Ok(status) => match status {
                        httparse::Status::Complete(offset) => {
                            self.offset = offset;
                            self.state = State::RequestFinished;
                            if let Ok((payload, keep_alive)) = self._iterate_headers(&req.headers) {
                                self.payload = payload;
                                self.keep_alive = keep_alive;
                                Event::Request
                            } else {
                                Event::RequestErr
                            }
                        }
                        httparse::Status::Partial => Event::PartialRequest,
                    },
                    Err(e) => {
                        println!("Parsing failed with \"{}\"", e);
                        Event::RequestErr
                    }
                }
            }
            State::RequestFinished => match self.payload {
                Payload::WebSocketUpgrade => todo!(),
                Payload::ChunkedPayload => todo!(),
                Payload::Payload(length) => {
                    self.state = State::Eof;
                    let data = self.next_data();
                    Event::Data(data.0)
                }
                Payload::None => {
                    self.state = State::Eof;
                    Event::Eof
                }
            },
            State::Eof => {
                match self.keep_alive {
                    KeepAlive::Close => {
                        self.state = State::Closed;
                        Event::Close
                    }
                    // HTTP/1.1 default is keep connection.
                    _ => {
                        self.state = State::Idle;
                        Event::Idle
                    }
                }
            }
            State::Closed => Event::Close,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_partial_request() {
        let mut conn = Http11Connection::new();

        conn.feed(b"GET /");
        assert!(matches!(dbg!(conn.next()), Event::PartialRequest))
    }

    #[test]
    fn test_get_request() {
        let mut conn = Http11Connection::new();

        conn.feed(b"GET /test HTTP/1.1\r\n\r\n");
        assert!(matches!(dbg!(conn.next()), Event::Request));
        assert!(matches!(conn.payload, Payload::None));
    }

    #[test]
    fn test_chunked() {
        let mut conn = Http11Connection::new();

        conn.feed(b"GET /test HTTP/1.1\r\nTransfer-Encoding:chunked\r\n\r\n");
        assert!(matches!(dbg!(conn.next()), Event::Request));
        assert!(matches!(conn.payload, Payload::ChunkedPayload));
    }

    #[test]
    fn test_post_request() {
        let mut conn = Http11Connection::new();

        conn.feed(b"POST /test HTTP/1.1\r\nContent-Length:1\r\n\r\na");
        assert!(matches!(dbg!(conn.next()), Event::Request));
        assert!(matches!(conn.payload, Payload::Payload(1)));

        let data = Bytes::from_static(b"a");
        assert!(matches!(conn.next(), Event::Data(data)));
    }

    #[test]
    fn test_content_length_duplicate() {
        let mut conn = Http11Connection::new();

        conn.feed(b"GET /test HTTP/1.1\r\nContent-Length:1\r\nContent-Length:1\r\n\r\n");
        assert!(matches!(dbg!(conn.next()), Event::RequestErr))
    }

    #[test]
    fn test_content_length_invalid() {
        let mut conn = Http11Connection::new();

        conn.feed(b"GET /test HTTP/1.1\r\nContent-Length:s\r\n\r\n");
        assert!(matches!(dbg!(conn.next()), Event::RequestErr))
    }

    #[test]
    fn test_tranfer_encoding_duplicate() {
        let mut conn = Http11Connection::new();

        conn.feed(
            b"GET /test HTTP/1.1\r\nTransfer-Encoding:chunked\r\nTransfer-Encoding:dup\r\n\r\n",
        );
        assert!(matches!(dbg!(conn.next()), Event::RequestErr))
    }

    #[test]
    fn test_content_length_with_chunked() {
        let mut conn = Http11Connection::new();

        conn.feed(b"GET /test HTTP/1.1\r\nContent-Length:1\r\nTransfer-Encoding:dup\r\n\r\n");
        assert!(matches!(dbg!(conn.next()), Event::RequestErr))
    }

    #[test]
    fn test_keep_alive() {
        let mut conn = Http11Connection::new();

        conn.feed(b"GET /test HTTP/1.1\r\nConnection: keep-alive\r\n\r\n");
        assert!(matches!(dbg!(conn.next()), Event::Request));
        assert!(matches!(dbg!(conn.next()), Event::Eof));
        assert!(matches!(dbg!(conn.next()), Event::Idle));
    }

    #[test]
    fn test_close_connection() {
        let mut conn = Http11Connection::new();

        conn.feed(b"GET /test HTTP/1.1\r\nConnection: close\r\n\r\n");
        assert!(matches!(dbg!(conn.next()), Event::Request));
        assert!(matches!(dbg!(conn.next()), Event::Eof));
        assert!(matches!(dbg!(conn.next()), Event::Close));
    }
}
