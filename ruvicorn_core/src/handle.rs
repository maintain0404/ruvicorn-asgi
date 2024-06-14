use bytes::BytesMut;

pub trait Handle<I, O, S, N, E> {
    fn step(&mut self, buffer: &mut BytesMut, state: S, inbound: I)
        -> Result<(O, S, Option<N>), E>;
}
