use bytes::{Bytes, BytesMut};

pub enum PayloadStepResult {
    Partial(Bytes, usize),
    Finished(Bytes, usize),
    Err,
}

pub trait Payload {
    fn step(&mut self, buffer: &mut BytesMut, offset: usize) -> PayloadStepResult;
}

#[derive(Debug)]
pub struct EmptyPayload {}

impl Payload for EmptyPayload {
    fn step(&mut self, buffer: &mut BytesMut, offset: usize) -> PayloadStepResult {
        PayloadStepResult::Finished(Bytes::new(), offset)
    }
}

#[derive(Debug)]
pub struct LengthedPayload {
    pub to_consume: usize,
}

impl LengthedPayload {
    fn new(length: usize) -> Self {
        Self { to_consume: length }
    }
}

impl Payload for LengthedPayload {
    fn step(&mut self, buffer: &mut BytesMut, offset: usize) -> PayloadStepResult {
        // Always to_comsume > 0
        let size_left = buffer.len() - offset;

        if size_left < self.to_consume {
            self.to_consume -= size_left;
            PayloadStepResult::Partial(
                buffer
                    .clone()
                    .split_off(offset)
                    .split_to(size_left)
                    .freeze(),
                offset + size_left,
            )
        } else if size_left > self.to_consume {
            PayloadStepResult::Err
        } else {
            self.to_consume = 0;
            PayloadStepResult::Finished(
                buffer
                    .clone()
                    .split_off(offset)
                    .split_to(size_left)
                    .freeze(),
                offset + size_left,
            )
        }
    }
}

#[derive(Debug)]
pub struct ChunkedPayload {}

impl Payload for ChunkedPayload {
    fn step(&mut self, buffer: &mut BytesMut, offset: usize) -> PayloadStepResult {
        todo!()
    }
}

#[derive(Debug)]
pub struct WebSocketUpgrade {}

impl Payload for WebSocketUpgrade {
    fn step(&mut self, buffer: &mut BytesMut, offset: usize) -> PayloadStepResult {
        todo!()
    }
}

#[derive(Debug)]
pub enum PayloadType {
    #[allow(dead_code)]
    WebSocketUpgrade(WebSocketUpgrade),
    ChunkedPayload(ChunkedPayload),
    LengthedPayload(LengthedPayload),
    None(EmptyPayload),
}

impl PayloadType {
    pub fn new_none() -> Self {
        Self::None(EmptyPayload {})
    }

    pub fn new_lengthed(length: usize) -> Self {
        Self::LengthedPayload(LengthedPayload { to_consume: length })
    }

    pub fn new_chunked() -> Self {
        Self::ChunkedPayload(ChunkedPayload {})
    }

    pub fn new_websocket_upgrade() -> Self {
        Self::WebSocketUpgrade(WebSocketUpgrade {})
    }
}

impl Payload for PayloadType {
    fn step(&mut self, buffer: &mut BytesMut, offset: usize) -> PayloadStepResult {
        match self {
            PayloadType::WebSocketUpgrade(p) => p.step(buffer, offset),
            PayloadType::ChunkedPayload(p) => p.step(buffer, offset),
            PayloadType::LengthedPayload(p) => p.step(buffer, offset),
            PayloadType::None(p) => p.step(buffer, offset),
        }
    }
}
