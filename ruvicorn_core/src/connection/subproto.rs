use bytes::BytesMut;


struct Http11Config {
    max_headers: usize,
}


pub trait SubProtocol {
    fn step_from(handler: impl SubProtocol) -> Self;
}

pub trait RecvSubProtocol: SubProtocol {
    type RecvEvent;

    fn recv(&mut self, buffer: &mut BytesMut, data: &[u8]) -> Self::RecvEvent;
}


pub trait SendSubProtocol: SubProtocol {
    type SendEvent;

    fn send(&mut self, buffer: &mut BytesMut, data: &[u8]) -> Self::SendEvent;
}

pub trait DuplexSubProtocol: RecvSubProtocol + SendSubProtocol {

}