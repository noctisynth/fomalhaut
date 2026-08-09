//! Greetd request/response transports.

use std::{future::Future, path::Path};

use greetd_ipc::{Request, Response, codec::TokioCodec};
use tokio::net::UnixStream;

use crate::TransportError;

/// Sequential request/response transport used by [`crate::GreeterClient`].
pub trait Transport: Send {
    /// Sends one request and waits for its corresponding response.
    fn exchange(
        &mut self,
        request: &Request,
    ) -> impl Future<Output = Result<Response, TransportError>> + Send;
}

/// Tokio Unix-socket implementation of the greetd IPC transport.
pub struct UnixTransport {
    stream: UnixStream,
}

impl UnixTransport {
    /// Connects to a greetd Unix socket.
    pub async fn connect(path: impl AsRef<Path>) -> Result<Self, TransportError> {
        let stream = UnixStream::connect(path)
            .await
            .map_err(TransportError::Connect)?;
        Ok(Self { stream })
    }
}

impl Transport for UnixTransport {
    async fn exchange(&mut self, request: &Request) -> Result<Response, TransportError> {
        request.write_to(&mut self.stream).await?;
        Response::read_from(&mut self.stream)
            .await
            .map_err(TransportError::from)
    }
}
