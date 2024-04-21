use bytes::Bytes;

pub struct AsgiReceiveRequest {
    body: Bytes,
    more_body: bool,
}
