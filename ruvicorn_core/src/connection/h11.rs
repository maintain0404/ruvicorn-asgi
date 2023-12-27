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
enum KeepAlive {
    KeepAlive,
    Close,
    None,
}

#[derive(Debug)]
enum UpgradeType {
    WebSocket,
}

#[derive(Debug)]
enum Payload {
    Upgrade(UpgradeType),
    ChunkedPayload,
    Payload,
    None,
}

#[derive(Debug)]
enum Event {
    PartialRequest,
    RequestErr,
    Request(Payload),
    Data(Bytes),
    ChunkedData(Bytes, bool),
    Eof,
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

struct Http11Connection {
    buffer: BytesMut,
    state: State,
    offset: usize,
    keep_alive: KeepAlive,
}

mod special_headers {
    pub const CONTENT_LENGTH: &str = "Content-Length";
    pub const TRANSFER_ENCODING: &str = "Transfer-Encoding";
    pub const CONNECTION: &str = "Connection";
    pub const UPGRADE: &str = "Upgrade";
}

impl Http11Connection {
    fn new() -> Self {
        Self {
            buffer: BytesMut::new(),
            state: State::Idle,
            offset: 0,
            keep_alive: KeepAlive::None,
        }
    }

    fn _iterate_headers(&self, headers: &[httparse::Header]) -> Event {
        let mut content_length = -1;

        let mut handled_te = false;
        let mut chunked = false;

        let mut keep_alive = KeepAlive::None;

        for header in headers.as_ref() {
            let name = header.name;

            let value = if let Ok(v) = std::str::from_utf8(header.value) {
                v
            } else {
                return Event::RequestErr;
            }
            .trim();

            if special_headers::CONTENT_LENGTH.eq_ignore_ascii_case(name) {
                // Content Length Header duplicate.
                if content_length != -1 || chunked {
                    test_trace!(
                        "h11.header.content-length-duplicate",
                        "Content-Length header duplicate or Transfer-Encoding is already set chunked."
                    );
                    return Event::RequestErr;
                }

                if let Ok(len) = value.parse::<i64>() {
                    content_length = len;
                } else {
                    test_trace!(
                        "h11.header.invalid-content-length",
                        "Invalid Content-Length Header"
                    );
                    return Event::RequestErr;
                }
            } else if special_headers::TRANSFER_ENCODING.eq_ignore_ascii_case(name) {
                if handled_te {
                    test_trace!(
                        "h11.header.transfer-encoding-duplicate",
                        "Tranfer-Encoding header is duplicated."
                    );
                    return Event::RequestErr;
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
                            return Event::RequestErr;
                        }
                        chunked = true;
                    } else if "identify".eq_ignore_ascii_case(eachv) {
                        // Pass
                    } else {
                        return Event::RequestErr;
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
            return Event::Request(Payload::Payload);
        } else if chunked {
            return Event::Request(Payload::ChunkedPayload);
        } else {
            return Event::Request(Payload::None);
        }
    }

    fn _next_chunked_data(&self) -> (Bytes, bool) {

        (Bytes::from_static(b""), false)
    }

    fn _data(&self) -> Bytes {
        Bytes::from_static(b"")
    }
 
    fn feed(&mut self, data: &[u8]) {
        self.buffer.extend(data);
    }

    fn next(&mut self) -> Event {
        match self.state {
            State::Idle => {
                let mut headers = [httparse::EMPTY_HEADER; 16];
                let mut req = httparse::Request::new(&mut headers);
                match req.parse(self.buffer.as_ref()) {
                    Ok(status) => match status {
                        httparse::Status::Complete(offset) => {
                            self.offset = offset;
                            self.state = State::RequestFinished;
                            let ev = self._iterate_headers(&req.headers);
                            if let Event::Request(payload) = &ev {
                                match payload {
                                    Payload::Upgrade(_) => {},
                                    Payload::ChunkedPayload => {},
                                    Payload::Payload => {},
                                    Payload::None => {},
                                }
                            }
                            ev
                        }
                        httparse::Status::Partial => Event::PartialRequest,
                    },
                    Err(e) => {
                        println!("Parsing failed with \"{}\"", e);
                        Event::RequestErr
                    }
                }
            }
            State::RequestFinished => Event::Data(Bytes::new()),
            State::Eof => {
                self.state = State::Idle;
                Event::Eof
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
        assert!(matches!(dbg!(conn.next()), Event::Request(Payload::None)))
    }

    #[test]
    fn test_chunked() {
        let mut conn = Http11Connection::new();

        conn.feed(b"GET /test HTTP/1.1\r\nTransfer-Encoding:chunked\r\n\r\n");
        assert!(matches!(
            dbg!(conn.next()),
            Event::Request(Payload::ChunkedPayload)
        ))
    }

    #[test]
    fn test_post_request() {
        let mut conn = Http11Connection::new();

        conn.feed(b"GET /test HTTP/1.1\r\nContent-Length:1\r\n\r\n");
        assert!(matches!(
            dbg!(conn.next()),
            Event::Request(Payload::Payload)
        ))
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
}
