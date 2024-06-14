use bytes::Bytes;

type Header = (Bytes, Bytes);

#[derive(Debug)]
pub struct RequestData {
    pub data: Bytes,
}

#[derive(Debug)]
pub struct ResponseStart {
    pub status: usize,
    pub headers: Vec<Header>,
}

#[derive(Debug)]
pub struct ResponseBody {
    pub body: Bytes,
    pub more_body: bool,
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum Inbound {
    // Feed data
    RequestData(RequestData),
    // Notice physical connection is closed.
    Disconnect,

    ResponseStart(ResponseStart),
    ResponseBody(ResponseBody),
}

#[derive(Debug)]
pub struct RequestHead {
    pub method: String,
    pub path: String,
    pub headers: Vec<Header>,
}

#[derive(Debug)]
pub struct RequestBody {
    pub body: Bytes,
    pub more_body: bool,
}

#[derive(Debug)]
pub struct ResponseData {
    pub data: Bytes,
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum Outbound {
    // Request is not finished or invalid.

    // Need more data to finish request.
    PartialRequest,
    // Request do not object HTTP spec.
    RequestErr,

    // Request is finished.

    // Request finished with Content-Length header.
    RequestHead(RequestHead),
    RequestBody(RequestBody),

    // Invalid response spec
    ReseponseErr,

    // Need more data to finish response
    PartialResponse,

    ResponseStart(ResponseData),
    ResponseBody(ResponseData),
}
