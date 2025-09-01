use kameo::message::StreamMessage;
use kameo::prelude::*;

#[cfg(feature = "pi")]
use futures::SinkExt;
#[cfg(feature = "pi")]
use futures::stream::{SplitSink, StreamExt};
#[cfg(feature = "pi")]
use tokio_serial::SerialPortBuilderExt;
#[cfg(feature = "pi")]
use tokio_serial::SerialStream;
#[cfg(feature = "pi")]
use tokio_util::codec::{Framed, LinesCodec};

use crate::toggl;

#[async_trait::async_trait]
pub trait Transmitter: Send {
    async fn send(
        &mut self,
        message: String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

#[cfg(feature = "pi")]
pub struct SerialTransmitter {
    inner: SplitSink<Framed<SerialStream, LinesCodec>, String>,
}

#[cfg(feature = "pi")]
#[async_trait::async_trait]
impl Transmitter for SerialTransmitter {
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

pub struct DummyTransmitter;

#[async_trait::async_trait]
impl Transmitter for DummyTransmitter {
    async fn send(
        &mut self,
        message: String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        tracing::info!("-> dummy send: {}", message);
        Ok(())
    }
}

pub struct Macropad {
    transmit: Box<dyn Transmitter>,
    toggl_ref: ActorRef<toggl::Toggl>,
}

#[derive(Debug)]
pub struct MacropadError;

impl Actor for Macropad {
    type Args = (ActorRef<toggl::Toggl>,);
    type Error = MacropadError;

    async fn on_start(state: Self::Args, _actor_ref: ActorRef<Self>) -> Result<Self, Self::Error> {
        #[cfg(feature = "pi")]
        {
            tracing::warn!("start open serial");
            let serial_port =
                tokio_serial::new(std::env::var("MACROPAD_SERIAL_PATH").unwrap(), 19200)
                    .open_native_async()
                    .unwrap();
            tracing::info!("serial opened");

            let device = Framed::new(serial_port, LinesCodec::new());
            let (transmit, receive) = device.split::<String>();

            _actor_ref.attach_stream(receive, (), ());

            let serial_transmitter = SerialTransmitter { inner: transmit };

            Ok(Self {
                transmit: Box::new(serial_transmitter),
                toggl_ref: state.0,
            })
        }

        #[cfg(not(feature = "pi"))]
        {
            Ok(Self {
                transmit: Box::new(DummyTransmitter),
                toggl_ref: state.0,
            })
        }
    }
}

impl Message<StreamMessage<Result<String, tokio_util::codec::LinesCodecError>, (), ()>>
    for Macropad
{
    type Reply = ();

    async fn handle(
        &mut self,
        message: StreamMessage<Result<String, tokio_util::codec::LinesCodecError>, (), ()>,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        match message {
            StreamMessage::Next(Ok(line)) => {
                if line == "h" {
                    return;
                }

                let message: serde_json::Value = serde_json::from_str(&line).unwrap();

                tracing::info!("<- message {:?}", message);
                self.process_command(message).await;
            }
            StreamMessage::Next(Err(e)) => {
                tracing::error!("! serial error: {}", e);
            }
            StreamMessage::Started(_) => {
                tracing::info!("= serial started");
            }
            StreamMessage::Finished(_) => {
                tracing::info!("= serial finished");
            }
        }
    }
}

impl Macropad {
    async fn process_command(&mut self, message: serde_json::Value) {
        let kind = message["kind"].as_str().unwrap();

        let result = match kind {
            "getTimeEntries" => self.send_time_entries().await,
            "startTimeEntry" => self.start_time_entry(message).await,
            "stopTimeEntry" => self.stop_time_entry().await,
            "continueTimeEntry" => self.continue_time_entry().await,
            "adjustTime" => self.adjust_time(message).await,
            _ => panic!("Unknown message kind: {}", kind),
        };

        match result {
            Ok(_) => self.send_success_message().await,
            Err(_) => self.send_error_message().await,
        }
    }

    async fn send_time_entries(&mut self) -> Result<(), ()> {
        let result = self
            .toggl_ref
            .ask(toggl::GetTimeEntries)
            .await
            .map_err(|_| ())?;

        for entry in result.as_array().unwrap() {
            self.send_message(serde_json::json!({
                "kind": "timeEntry",
                "timeEntry": {
                    "description": entry["description"].as_str().unwrap(),
                }
            }))
            .await;
        }

        Ok(())
    }

    async fn start_time_entry(&mut self, message: serde_json::Value) -> Result<(), ()> {
        self.toggl_ref
            .ask(toggl::StartTimeEntry {
                description: message["timeEntry"]["description"]
                    .as_str()
                    .unwrap()
                    .to_string(),
            })
            .await
            .map_err(|_| ())?;

        Ok(())
    }

    async fn stop_time_entry(&mut self) -> Result<(), ()> {
        self.toggl_ref
            .ask(toggl::StopTimeEntry)
            .await
            .map_err(|_| ())?;

        Ok(())
    }

    async fn continue_time_entry(&mut self) -> Result<(), ()> {
        self.toggl_ref
            .ask(toggl::ContinueTimeEntry)
            .await
            .map_err(|_| ())?;

        Ok(())
    }

    async fn adjust_time(&mut self, message: serde_json::Value) -> Result<(), ()> {
        self.toggl_ref
            .ask(toggl::AdjustTime {
                minutes: message["minutes"].as_i64().unwrap(),
            })
            .await
            .map_err(|_| ())?;

        Ok(())
    }

    async fn send_success_message(&mut self) {
        let reply = serde_json::json!({ "kind": "success" });
        self.send_message(reply).await;
    }

    async fn send_error_message(&mut self) {
        let reply = serde_json::json!({ "kind": "error" });
        self.send_message(reply).await;
    }

    async fn send_message(&mut self, message: serde_json::Value) {
        tracing::info!("-> message {:?}", message);
        self.transmit.send(message.to_string()).await.unwrap();
    }
}
