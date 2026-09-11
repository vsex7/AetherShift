use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use serde::{Serialize, de::DeserializeOwned};
use std::marker::PhantomData;
use std::path::Path;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::UnixStream;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

use crate::error::ProtocolError;
use crate::types::{Request, Response};

/// Max allowed message size (16 MB)
pub const MAX_FRAME_LENGTH: usize = 16 * 1024 * 1024;

/// Creates a new `LengthDelimitedCodec` configured with a 4-byte length prefix.
pub fn new_length_delimited_codec() -> LengthDelimitedCodec {
    LengthDelimitedCodec::builder()
        .length_field_length(4)
        .max_frame_length(MAX_FRAME_LENGTH)
        .new_codec()
}

/// A framed asynchronous stream that encodes outgoing messages and decodes incoming messages
/// using 4-byte length-delimited framing and JSON serialization.
pub struct ProtocolStream<S, In, Out> {
    framed: Framed<S, LengthDelimitedCodec>,
    _marker: PhantomData<(In, Out)>,
}

impl<S, In, Out> ProtocolStream<S, In, Out>
where
    S: AsyncRead + AsyncWrite + Unpin,
    In: DeserializeOwned,
    Out: Serialize,
{
    /// Wraps an underlying asynchronous I/O stream with length-delimited JSON framing.
    pub fn new(stream: S) -> Self {
        let codec = new_length_delimited_codec();
        Self {
            framed: Framed::new(stream, codec),
            _marker: PhantomData,
        }
    }

    /// Receives and decodes the next message from the stream.
    pub async fn recv(&mut self) -> Result<In, ProtocolError> {
        match self.framed.next().await {
            Some(Ok(bytes)) => {
                let msg = serde_json::from_slice::<In>(&bytes)?;
                Ok(msg)
            }
            Some(Err(e)) => Err(ProtocolError::Frame(e.to_string())),
            None => Err(ProtocolError::ConnectionClosed),
        }
    }

    /// Encodes and sends a message through the stream.
    pub async fn send(&mut self, item: &Out) -> Result<(), ProtocolError> {
        let serialized = serde_json::to_vec(item)?;
        self.framed
            .send(Bytes::from(serialized))
            .await
            .map_err(|e| ProtocolError::Frame(e.to_string()))?;
        Ok(())
    }

    /// Consumes this wrapper and returns the underlying framed stream.
    pub fn into_inner(self) -> Framed<S, LengthDelimitedCodec> {
        self.framed
    }

    /// Access reference to the underlying framed stream.
    pub fn get_ref(&self) -> &Framed<S, LengthDelimitedCodec> {
        &self.framed
    }

    /// Access mutable reference to the underlying framed stream.
    pub fn get_mut(&mut self) -> &mut Framed<S, LengthDelimitedCodec> {
        &mut self.framed
    }
}

/// Convenient stream type for clients: sends `Request`, receives `Response`.
pub type ClientStream<S = UnixStream> = ProtocolStream<S, Response, Request>;

/// Convenient stream type for servers: receives `Request`, sends `Response`.
pub type ServerStream<S = UnixStream> = ProtocolStream<S, Request, Response>;

/// Convenience function to read a single `Request` from any length-delimited framed stream.
pub async fn read_request<S>(
    framed: &mut Framed<S, LengthDelimitedCodec>,
) -> Result<Request, ProtocolError>
where
    S: AsyncRead + Unpin,
{
    match framed.next().await {
        Some(Ok(bytes)) => {
            let req = serde_json::from_slice::<Request>(&bytes)?;
            Ok(req)
        }
        Some(Err(e)) => Err(ProtocolError::Frame(e.to_string())),
        None => Err(ProtocolError::ConnectionClosed),
    }
}

/// Convenience function to send a single `Request` to any length-delimited framed stream.
pub async fn send_request<S>(
    framed: &mut Framed<S, LengthDelimitedCodec>,
    request: &Request,
) -> Result<(), ProtocolError>
where
    S: AsyncWrite + Unpin,
{
    let bytes = serde_json::to_vec(request)?;
    framed
        .send(Bytes::from(bytes))
        .await
        .map_err(|e| ProtocolError::Frame(e.to_string()))?;
    Ok(())
}

/// Convenience function to read a single `Response` from any length-delimited framed stream.
pub async fn read_response<S>(
    framed: &mut Framed<S, LengthDelimitedCodec>,
) -> Result<Response, ProtocolError>
where
    S: AsyncRead + Unpin,
{
    match framed.next().await {
        Some(Ok(bytes)) => {
            let resp = serde_json::from_slice::<Response>(&bytes)?;
            Ok(resp)
        }
        Some(Err(e)) => Err(ProtocolError::Frame(e.to_string())),
        None => Err(ProtocolError::ConnectionClosed),
    }
}

/// Convenience function to send a single `Response` to any length-delimited framed stream.
pub async fn send_response<S>(
    framed: &mut Framed<S, LengthDelimitedCodec>,
    response: &Response,
) -> Result<(), ProtocolError>
where
    S: AsyncWrite + Unpin,
{
    let bytes = serde_json::to_vec(response)?;
    framed
        .send(Bytes::from(bytes))
        .await
        .map_err(|e| ProtocolError::Frame(e.to_string()))?;
    Ok(())
}

/// Client helper: connects to a Unix domain socket at `path`, sends a `Request`, and waits for a `Response`.
pub async fn call(path: impl AsRef<Path>, request: &Request) -> Result<Response, ProtocolError> {
    let stream = UnixStream::connect(path).await?;
    let mut client = ClientStream::new(stream);
    client.send(request).await?;
    client.recv().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::StatusInfo;
    use tokio::io::duplex;

    #[tokio::test]
    async fn test_protocol_stream_request_response() {
        let (client_io, server_io) = duplex(1024);

        let mut client = ClientStream::new(client_io);
        let mut server = ServerStream::new(server_io);

        let req = Request::Switch {
            profile: "dark".to_string(),
            force: false,
        };

        // Client sends Request
        client.send(&req).await.expect("client send");

        // Server receives Request
        let received_req = server.recv().await.expect("server recv");
        assert_eq!(received_req, req);

        // Server sends Response
        let resp = Response::ok("Switched successfully");
        server.send(&resp).await.expect("server send");

        // Client receives Response
        let received_resp = client.recv().await.expect("client recv");
        assert_eq!(received_resp, resp);
    }

    #[tokio::test]
    async fn test_convenience_functions() {
        let (client_io, server_io) = duplex(1024);

        let mut client_framed = Framed::new(client_io, new_length_delimited_codec());
        let mut server_framed = Framed::new(server_io, new_length_delimited_codec());

        let req = Request::Status;
        send_request(&mut client_framed, &req)
            .await
            .expect("send_request");

        let received_req = read_request(&mut server_framed)
            .await
            .expect("read_request");
        assert_eq!(received_req, req);

        let status_info = StatusInfo::new("nord", 1, 42, "0.1.0");
        let resp = Response::ok_with_data("Status report", &status_info).expect("ok_with_data");
        send_response(&mut server_framed, &resp)
            .await
            .expect("send_response");

        let received_resp = read_response(&mut client_framed)
            .await
            .expect("read_response");
        assert_eq!(received_resp, resp);
    }

    #[tokio::test]
    async fn test_connection_closed() {
        let (client_io, server_io) = duplex(1024);

        let mut server = ServerStream::new(server_io);
        drop(client_io);

        let err = server.recv().await.expect_err("should be error");
        match err {
            ProtocolError::ConnectionClosed => {}
            other => panic!("expected ConnectionClosed, got {:?}", other),
        }
    }
}
