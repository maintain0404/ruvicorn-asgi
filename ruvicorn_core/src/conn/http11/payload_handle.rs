use bytes::Bytes;

use crate::handle::Handle;

use super::{
    bound::{RequestBody, RequestData},
    state::State,
};

#[derive(Debug)]
pub struct EmptyPayload {}

impl Handle<RequestData, RequestBody, State, (), ()> for EmptyPayload {
    fn step(
        &mut self,
        buffer: &mut bytes::BytesMut,
        state: State,
        _: RequestData,
    ) -> Result<(RequestBody, State, ()), ((), State)> {
        buffer.clear();
        Ok((
            RequestBody {
                body: buffer.clone().freeze(),
                more_body: false,
            },
            state,
            (),
        ))
    }
}

#[derive(Debug)]
pub struct LengthedPayload {
    pub remaining: usize,
}

impl Handle<RequestData, RequestBody, State, (), ()> for LengthedPayload {
    fn step(
        &mut self,
        buffer: &mut bytes::BytesMut,
        state: State,
        inbound: RequestData,
    ) -> Result<(RequestBody, State, ()), ((), State)> {
        debug_assert!(matches!(state, State::RequestHeadFinished));
        let buf;
        buffer.extend(inbound.data);

        if buffer.len() < self.remaining {
            if buffer.is_empty() {
                buf = Bytes::new();
            } else {
                buf = buffer.split().freeze();
                self.remaining -= buf.len();
            }
            Ok((
                RequestBody {
                    body: buf,
                    more_body: true,
                },
                State::RequestHeadFinished,
                (),
            ))
        } else if buffer.len() > self.remaining {
            Err(((), State::Closed))
        } else {
            Ok((
                RequestBody {
                    body: buffer.split().freeze(),
                    more_body: false,
                },
                State::RequestBodyFinished,
                (),
            ))
        }
    }
}

#[derive(Debug)]
pub enum PayloadType {
    Lenghthed(LengthedPayload),
    Chunked,
}

#[cfg(test)]
mod test {
    use bytes::BytesMut;

    use super::*;

    #[test]
    fn test_empty_payload() {
        let mut payload = EmptyPayload {};
        let mut buffer = BytesMut::new();

        let (body, next_state, _) = payload
            .step(
                &mut buffer,
                State::RequestHeadFinished,
                RequestData {
                    data: Bytes::from_static(b"data"),
                },
            )
            .unwrap();

        assert_eq!(body.body, Bytes::new());
        assert_eq!(body.more_body, false);
        assert!(buffer.is_empty());
        assert!(matches!(next_state, State::RequestHeadFinished));
    }

    #[test]
    fn test_lengthed_payload() {
        let size = 4;
        let mut payload = LengthedPayload { remaining: size };
        let mut buffer = BytesMut::new();

        let (body, next_state, _) = payload
            .step(
                &mut buffer,
                State::RequestHeadFinished,
                RequestData {
                    data: Bytes::from_static(b"data"),
                },
            )
            .unwrap();

        assert_eq!(body.body, Bytes::from_static(b"data"));
        assert_eq!(body.more_body, false);
        assert!(buffer.is_empty());
        assert!(matches!(next_state, State::RequestBodyFinished));
    }

    #[test]
    fn test_lengthed_payload_with_not_enough_data() {
        let size = 8;
        let mut payload = LengthedPayload { remaining: size };
        let mut buffer = BytesMut::new();

        let (body, next_state, _) = payload
            .step(
                &mut buffer,
                State::RequestHeadFinished,
                RequestData {
                    data: Bytes::from_static(b"data"),
                },
            )
            .unwrap();

        assert_eq!(body.body, Bytes::from_static(b"data"));
        assert_eq!(body.more_body, true);
        assert_eq!(payload.remaining, 4);
        assert!(buffer.is_empty());
        assert!(matches!(next_state, State::RequestHeadFinished));
    }

    #[test]
    fn test_lengthed_payload_with_too_much_data() {
        let size = 4;
        let mut payload = LengthedPayload { remaining: size };
        let mut buffer = BytesMut::new();

        payload
            .step(
                &mut buffer,
                State::RequestHeadFinished,
                RequestData {
                    data: Bytes::from_static(b"testdata"),
                },
            )
            .unwrap_err();
    }
}
