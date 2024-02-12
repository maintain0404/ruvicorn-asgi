
use crate::connection::subproto::SubProtocol;
use bytes::{Buf, Bytes, BytesMut};
use httparse;

use super::subproto::RecvSubProtocol;

const HOST: &str = "Host";
const CONNECTION: &str = "Connection";
const TRANSFER_ENCODING: &str = "Transfer-Encoding";
const CONTENT_LENGTH: &str = "Content-Length";
const UPGRADE: &str = "Upgrade";
const MAX_HEADERS: usize = 96;

enum BodyType {
    Length { length: u64 },
    None,
    Chunked,
}

#[derive(Debug)]
enum HeadEvent {
    // Request is not finished or invalid.
    Partial,
    ParseErr,
    RequestErr,

    // Request is finished.
    LengthedBody {
        method: String,
        path: String,
        headers: Vec<(Bytes, Bytes)>,
        host: String,
        keep_alive: bool,
        content_length: u64,
    },
    NoBody {
        method: String,
        path: String,
        host: String,
        headers: Vec<(Bytes, Bytes)>,
        keep_alive: bool,
    },
    // Will be implemented later.
    ChunkedBody {
        method: String,
        path: String,
        headers: Vec<(Bytes, Bytes)>,
        host: String,
        keep_alive: bool,
    },
    // UpgradeWebsocket will be added.
}

impl HeadEvent {
    fn is_finished(&self) -> bool {
        return match self {
            HeadEvent::Partial => false,
            HeadEvent::ParseErr => false,
            HeadEvent::RequestErr => false,
            _ => true,
        };
    }
}

struct RequestHead {}

impl Default for RequestHead {
    fn default() -> Self {
        Self {}
    }
}

impl SubProtocol for RequestHead {
}

impl RecvSubProtocol for RequestHead {
    type RecvEvent = HeadEvent;

    fn recv(&mut self, buffer: &mut BytesMut, data: &[u8]) -> Self::RecvEvent {
        buffer.extend_from_slice(data);
        let mut headers = [httparse::EMPTY_HEADER; 96];
        let mut req = httparse::Request::new(&mut headers);
        let parsed = req.parse(buffer);
        let body_start_from: usize;

        match parsed {
            Ok(status) => match status {
                httparse::Status::Complete(offset) => {
                    body_start_from = offset;
                }
                httparse::Status::Partial => {
                    return Self::RecvEvent::Partial;
                }
            },
            Err(_) => {
                return Self::RecvEvent::ParseErr;
            }
        }

        let method: String;
        match req.method {
            Some(m) => {
                method = String::from(m);
            }
            None => {
                return Self::RecvEvent::RequestErr;
            }
        }

        let path: String;
        match req.path {
            Some(p) => {
                path = String::from(p);
            }
            None => {
                return Self::RecvEvent::RequestErr;
            }
        }

        match self.iterate_and_check_headers(buffer, &headers) {
            Ok((bytes_header, host, body_type, keep_alive)) => {
                buffer.advance(body_start_from);
                match body_type {
                BodyType::Length { length } => {
                    return Self::RecvEvent::LengthedBody {
                        method: method,
                        path: path,
                        headers: bytes_header,
                        host: host,
                        keep_alive: keep_alive,
                        content_length: length,
                    }
                }
                BodyType::None => {
                    return Self::RecvEvent::NoBody {
                        method: method,
                        path: path,
                        headers: bytes_header,
                        host: host,
                        keep_alive: keep_alive,
                    }
                }
                BodyType::Chunked => {
                    return Self::RecvEvent::ChunkedBody {
                        method: method,
                        path: path,
                        headers: bytes_header,
                        host: host,
                        keep_alive: keep_alive,
                    }
                }
            }},
            Err(_) => {
                return Self::RecvEvent::RequestErr;
            }
        }
    }
}

impl RequestHead {
    fn new() -> Self {
        Self {}
    }

    // cast httparse header into bytes tuple.
    // TODO: figure out more perfomant way without clone.
    fn header_into_bytes(&self, buffer: &BytesMut, name: &str, value: &[u8]) -> (Bytes, Bytes) {
        let buf_ptr = buffer.as_ptr() as usize;

        let name_ptr = name.as_ptr() as usize;
        dbg!(buffer, name_ptr, buf_ptr);
        let name_bytes = buffer
            .clone()
            .split_off(name_ptr - buf_ptr)
            .split_to(name.len())
            .freeze();

        let value_ptr = value.as_ptr() as usize;
        let value_bytes = buffer
            .clone()
            .split_off(value_ptr - buf_ptr)
            .split_to(name.len())
            .freeze();

        return (name_bytes, value_bytes);
    }

    // Iterate all headers, check special connection-releated headers, and cast as Bytes tuple.
    // Bytes tuple will be passed as asgi scope dict.
    fn iterate_and_check_headers(
        &mut self,
        buffer: &BytesMut,
        headers: &[httparse::Header<'_>; MAX_HEADERS],
    ) -> Result<(Vec<(Bytes, Bytes)>, String, BodyType, bool), ()> {
        let mut content_length: Option<u64> = Option::None;
        let mut host_opt = Option::None;
        let mut handled_te = false;
        let mut chunked = false;
        let mut keep_alive = true; // HTTP/1.1 Default
        let mut bytes_headers: Vec<(Bytes, Bytes)> = Vec::with_capacity(MAX_HEADERS);

        for header in headers.iter() {
            if header.name.is_empty() {
                break;
            }
            let bytes_header = self.header_into_bytes(buffer, header.name, header.value);
            bytes_headers.push(bytes_header);

            let name = header.name;
            let value = if let Ok(v) = std::str::from_utf8(header.value) {
                v
            } else {
                dbg!("Header value parsing failed.");
                return Result::Err(());
            }
            .trim();

            match name {
                name if HOST.eq_ignore_ascii_case(name) => {
                    match host_opt {
                        // Host header duplicate.
                        Some(_) => {
                            dbg!("Host header duplicate.");
                            return Result::Err(());
                        }
                        None => {
                            host_opt = Option::from(name.to_owned());
                        }
                    }
                }
                name if CONNECTION.eq_ignore_ascii_case(name) => {
                    keep_alive = if "keep-alive".eq_ignore_ascii_case(value) {
                        true
                    } else if "close".eq_ignore_ascii_case(value) {
                        false
                    } else {
                        true
                    }
                }
                name if true == CONTENT_LENGTH.eq_ignore_ascii_case(name) => match content_length {
                    // Content-Length header duplicate.
                    Some(_) => {
                        dbg!("Content-Length header duplicate.");
                        return Result::Err(());
                    }
                    // Try parsing Content-Length Header.
                    None => match value.parse::<u64>() {
                        Ok(v) => {
                            // Chunked and Content-Length can be used either.
                            if chunked {
                                return Result::Err(());
                            }
                            content_length = Some(v)
                        }
                        Err(_) => return Result::Err(()),
                    },
                },
                name if true == TRANSFER_ENCODING.eq_ignore_ascii_case(name) => {
                    if handled_te {
                        dbg!("Transfer-Encoding header duplicate.");
                        return Result::Err(());
                    }

                    for eachv in value.split(',').map(str::trim) {
                        if "chunked".eq_ignore_ascii_case(eachv) {
                            if let Some(_) = content_length {
                                return Result::Err(());
                            }
                            chunked = true;
                        } else {
                            dbg!("Transfer-Encoding header value duplicate.");
                            return Result::Err(());
                        }
                    }
                    handled_te = true;
                }
                _ => {}
            }
        }

        let host;
        match host_opt {
            Some(v) => {
                host = v;
            }
            None => return Result::Err(()),
        }

        match content_length {
            Some(length) => Result::Ok((
                bytes_headers,
                host,
                BodyType::Length { length: length },
                keep_alive,
            )),
            None if chunked => Result::Ok((bytes_headers, host, BodyType::Chunked, keep_alive)),
            _ => Result::Ok((bytes_headers, host, BodyType::None, keep_alive)),
        }
    }
}


// Request body
#[derive(Debug)]
enum LengthedBodyEvent {
    Partial,
    Complete(Bytes),
    ToLong,
}

#[derive(Debug)]
struct LengthedBody {
    length: usize
}

impl SubProtocol for LengthedBody {
}

impl RecvSubProtocol for LengthedBody {
    type RecvEvent = LengthedBodyEvent;

    fn recv(&mut self, buffer: &mut BytesMut, data: &[u8]) -> Self::RecvEvent {
        buffer.extend_from_slice(data);

        if buffer.len() == self.length {
            Self::RecvEvent::Complete(buffer.clone().freeze())
        } else if buffer.len() < self.length {
            Self::RecvEvent::Partial
        } else {
            Self::RecvEvent::ToLong
        }
    }
}

impl LengthedBody {
    fn new(length: usize) -> Self {
        Self { length: length }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_partial_request() {
        let mut conn = RequestHead::new();
        let mut buffer = BytesMut::new();

        let ev = dbg!(conn.recv(&mut buffer, b"GET /"));
        assert!(matches!(ev, HeadEvent::Partial))
    }

    #[test]
    fn test_get_request() {
        let mut conn = RequestHead::new();
        let mut buffer = BytesMut::new();

        let ev = dbg!(conn.recv(&mut buffer, b"GET /test HTTP/1.1\r\nHost:localhost\r\n\r\n"));
        assert!(matches!(ev, HeadEvent::NoBody { .. }));
    }

    #[test]
    fn test_chunked() {
        let mut conn = RequestHead::new();
        let mut buffer = BytesMut::new();

        let ev = dbg!(conn.recv(
            &mut buffer,
            b"GET /test HTTP/1.1\r\nTransfer-Encoding:chunked\r\nHost:localhost\r\n\r\n"
        ));
        assert!(matches!(ev, HeadEvent::ChunkedBody { .. }));
    }

    #[test]
    fn test_post_request() {
        let mut conn = RequestHead::new();
        let mut buffer = BytesMut::new();

        let ev = dbg!(conn.recv(
            &mut buffer,
            b"POST /test HTTP/1.1\r\nContent-Length:1\r\nHost:localhost\r\n\r\na"
        ),);
        assert!(matches!(
            ev,
            HeadEvent::LengthedBody {
                content_length: 1,
                ..
            }
        ));
    }

    #[test]
    fn test_content_length_duplicate() {
        let mut conn = RequestHead::new();
        let mut buffer = BytesMut::new();

        let ev = dbg!(conn.recv(
            &mut buffer,
            b"GET /test HTTP/1.1\r\nContent-Length:1\r\nContent-Length:1\r\nHost:localhost\r\n\r\n"
        ));
        assert!(matches!(ev, HeadEvent::RequestErr))
    }

    #[test]
    fn test_content_length_invalid() {
        let mut conn = RequestHead::new();
        let mut buffer = BytesMut::new();

        let ev = dbg!(conn.recv(
            &mut buffer,
            b"GET /test HTTP/1.1\r\nContent-Length:s\r\nHost:localhost\r\n\r\n"
        ));
        assert!(matches!(ev, HeadEvent::RequestErr))
    }

    #[test]
    fn test_tranfer_encoding_duplicate() {
        let mut conn = RequestHead::new();
        let mut buffer = BytesMut::new();

        let ev = dbg!(conn.recv(
            &mut buffer,
            b"GET /test HTTP/1.1\r\nTransfer-Encoding:chunked\r\nTransfer-Encoding:dup\r\nHost:localhost\r\n\r\n",
        ));
        assert!(matches!(ev, HeadEvent::RequestErr))
    }

    #[test]
    fn test_content_length_with_chunked() {
        let mut conn = RequestHead::new();
        let mut buffer = BytesMut::new();

        let ev = dbg!(conn.recv(
            &mut buffer,
            b"GET /test HTTP/1.1\r\nContent-Length:1\r\nTransfer-Encoding:dup\r\nHost:localhost\r\n\r\n"
        ));
        assert!(matches!(ev, HeadEvent::RequestErr));
    }

    #[test]
    fn test_keep_alive() {
        let mut conn = RequestHead::new();
        let mut buffer = BytesMut::new();

        let ev = dbg!(conn.recv(
            &mut buffer,
            b"GET /test HTTP/1.1\r\nConnection: keep-alive\r\nHost:localhost\r\n\r\n"
        ));
        assert!(matches!(
            ev,
            HeadEvent::NoBody {
                keep_alive: true,
                ..
            }
        ));
    }

    #[test]
    fn test_close_connection() {
        let mut conn = RequestHead::new();
        let mut buffer = BytesMut::new();

        let ev = dbg!(conn.recv(
            &mut buffer,
            b"GET /test HTTP/1.1\r\nConnection: close\r\nHost:localhost\r\n\r\n"
        ));
        assert!(matches!(
            ev,
            HeadEvent::NoBody {
                keep_alive: false,
                ..
            }
        ));
    }

    //TODO! test when headers are more than MAX_HEADERS.

    #[test]
    fn test_lengthed_body_precise_size() {
        let mut conn = LengthedBody::new(12);
        let mut buffer = BytesMut::new();
        let ev = dbg!(conn.recv(
            &mut buffer,
            b"Hello World!"
        ));

        assert!(matches!(
            ev,
            LengthedBodyEvent::Complete(_)
        ));

        if let LengthedBodyEvent::Complete(data) = ev {
            assert_eq!(data, Bytes::from_static(b"Hello World!"))
        }
    }

    #[test]
    fn test_lengthed_body_partial() {
        let mut conn = LengthedBody::new(15);
        let mut buffer = BytesMut::new();
        let ev = dbg!(conn.recv(
            &mut buffer,
            b"Hello World!"
        ));

        assert!(matches!(
            ev,
            LengthedBodyEvent::Partial
        ));
    }

    #[test]
    fn test_lengthed_body_too_long() {
        let mut conn = LengthedBody::new(10);
        let mut buffer = BytesMut::new();
        let ev = dbg!(conn.recv(
            &mut buffer,
            b"Hello World!"
        ));

        assert!(matches!(
            ev,
            LengthedBodyEvent::ToLong
        ));
    }
}
