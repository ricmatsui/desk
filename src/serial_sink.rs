#[cfg(feature = "pi")]
use futures::SinkExt;
#[cfg(feature = "pi")]
use futures::stream::{SplitSink};
#[cfg(feature = "pi")]
use tokio_serial::SerialStream;
#[cfg(feature = "pi")]
use tokio_util::codec::{Framed, LinesCodec};

#[async_trait::async_trait]
pub trait Sink: Send {
    async fn send(
        &mut self,
        message: String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

#[cfg(feature = "pi")]
pub struct SerialSink {
    inner: SplitSink<Framed<SerialStream, LinesCodec>, String>,
}

#[cfg(feature = "pi")]
impl SerialSink {
    pub fn new(inner: SplitSink<Framed<SerialStream, LinesCodec>, String>) -> Self {
        Self { inner }
    }
}

#[cfg(feature = "pi")]
#[async_trait::async_trait]
impl Sink for SerialSink {
    async fn send(
        &mut self,
        message: String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.inner
            .send(message)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }
}

pub struct DummySink;

#[async_trait::async_trait]
impl Sink for DummySink {
    async fn send(
        &mut self,
        message: String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        tracing::info!("-> dummy send: {}", message);
        Ok(())
    }
}

