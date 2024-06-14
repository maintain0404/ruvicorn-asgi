#[derive(Debug)]
pub enum State {
    // Ready to get request.
    Idle,
    // Request parse finished. Ready for get body.
    RequestHeadFinished,
    // Get all requet body data. Ready to response.
    RequestBodyFinished,
    // Response Head parse finished. Ready for send body.
    ResponseHeadFinished,
    // Connection closed by error or finished all request/response cycle.
    Closed,
}
