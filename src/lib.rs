use alloy::{
    node_bindings::Anvil,
    primitives::U256,
    providers::{Provider, ProviderBuilder},
    rpc::{
        client::ClientBuilder,
        json_rpc::{RequestPacket, ResponsePacket},
    },
    transports::TransportError,
    uint,
};

use std::{
    fmt::Debug,
    future::{Future, IntoFuture},
    pin::Pin,
    task::{Context, Poll},
};

pub static ONE_ETHER: U256 = uint!(1_000_000_000_000_000_000_U256);
pub static GWEI: U256 = uint!(1_000_000_000_U256);

pub static GWEI_I: u128 = 1_000_000_000;
use tower::{Layer, Service};

pub struct DeadbeefLayer;

impl<S> Layer<S> for DeadbeefLayer {
    type Service = DeadbeefService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        DeadbeefService { inner }
    }
}

#[derive(Debug, Clone)]
pub struct DeadbeefService<S> {
    inner: S,
}

impl<S> Service<RequestPacket> for DeadbeefService<S>
where
    // Constraints on the service.
    S: Service<RequestPacket, Response = ResponsePacket, Error = TransportError>,
    S::Future: Send + 'static,
    S::Response: Send + 'static + Debug,
    S::Error: Send + 'static + Debug,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: RequestPacket) -> Self::Future {
        println!("Request: {req:?}");

        let fut = self.inner.call(req);

        Box::pin(fut)
    }
}
