use bytes::{BytesMut, Bytes};

use crate::handle::Handle;

use super::{
    bound::{RequestData, RequestHead}, payload_handle::PayloadType, state::State
};

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

pub struct ConnectionInfo {
    keep_alive: KeepAlive,
    payload_type: PayloadType,
}

#[derive(Debug)]
enum HeaderError {
    Duplicate,
    InvalidValue,
}

#[derive(Debug)]
pub enum RequestError {
    InvalidRequest,
    InvalidHeader,
    PartialRequest
}

impl From<HeaderError> for RequestError {
    fn from(value: HeaderError) -> Self {
        Self::InvalidHeader
    }
}

#[derive(Debug)]
pub struct Request {
    content_length: u64,
    chunked: bool,
    keep_alive: KeepAlive
}

impl Request {
    fn cast_header(buffer: &BytesMut, header: &httparse::Header) -> Bytes {
        todo!();
    }

    fn parse_content_length_header(&self, value: &str) -> Result<u64, HeaderError> {
        if let Ok(len) = value.parse::<u64>() {
            return Ok(len);
        } else {
            return Err(HeaderError::InvalidValue);
        }
        todo!()
    } 

    fn parse_connection_header(&self, value: &str) -> Result<KeepAlive, HeaderError> {
        todo!()
    }

    fn parse_transfer_encoding_header(&self, value: &str) -> Result<bool, HeaderError> {
        todo!()
    }

    fn iterate_headers(&self, buffer: &BytesMut, headers: &[httparse::Header]) -> Result<(ConnectionInfo, Vec<(Bytes, Bytes)>), HeaderError>{
        let mut content_length: Option<u64> = None;

        let mut handled_te = false;
        let mut chunked = false;

        let mut keep_alive = KeepAlive::None;

        for header in headers.as_ref() {
            let name = header.name;
            let value = if let Ok(v) = std::str::from_utf8(header.value) {
                v 
            } else {
                return Result::Err(HeaderError::InvalidValue)
            }.trim();

            if special_headers::CONTENT_LENGTH.eq_ignore_ascii_case(name) {
                content_length = Some(self.parse_content_length_header(value)?);
            } else if special_headers::TRANSFER_ENCODING.eq_ignore_ascii_case(name) {
                chunked = self.parse_transfer_encoding_header(value)?;
            } else if special_headers::CONNECTION.eq_ignore_ascii_case(name) {
                keep_alive = self.parse_connection_header(value)?;
            }
        }

        

        todo!();
    }

    fn parse(&mut self, buffer: &mut BytesMut) -> Result<(RequestHead, ConnectionInfo), RequestError> {
        let mut headers = [httparse::EMPTY_HEADER; 96];
        let mut req = httparse::Request::new(&mut headers);
        
        match req.parse(buffer.as_ref()) {
            Ok(status) => match status {
                httparse::Status::Complete(_) => {
                    let cloned = buffer.clone();
                    let (info, headers) = self.iterate_headers(&cloned, &req.headers)?;
                    return Ok((
                        RequestHead {
                            method: req.method.unwrap().to_owned(),
                            path: req.path.unwrap().to_owned(),
                            headers: headers
                        },
                        info
                    ))
                },
                httparse::Status::Partial => return Err(RequestError::PartialRequest),
            },
            Err(_) => return Err(RequestError::InvalidRequest),
        }
    }
}

impl Handle<RequestData, RequestHead, State, ConnectionInfo, ()> for Request {
    fn step(
        &mut self,
        buffer: &mut BytesMut,
        state: State,
        inbound: RequestData,
    ) -> Result<(RequestHead, State, Option<ConnectionInfo>), ()> {
        debug_assert!(matches!(state, State::Idle));
        buffer.extend_from_slice(&inbound.data);
        match self.parse(buffer) {
            Ok((head, info)) => {
                Ok((head, State::RequestHeadFinished, Some(info)))
            },
            Err(_) => Err(()),
        }
    }
}
