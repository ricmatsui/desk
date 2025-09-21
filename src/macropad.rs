use kameo::message::StreamMessage;
use kameo::prelude::*;
use kameo_actors::broker;

#[cfg(feature = "pi")]
use futures::stream::StreamExt;
#[cfg(feature = "pi")]
use tokio_serial::SerialPortBuilderExt;
#[cfg(feature = "pi")]
use tokio_util::codec::{Framed, LinesCodec};

use crate::toggl;

pub struct Macropad {
    transmit: Box<dyn crate::serial_sink::Sink>,
    broker_ref: ActorRef<broker::Broker<crate::BrokerMessage>>,
    toggl_ref: ActorRef<toggl::Toggl>,
}

#[derive(Debug)]
pub struct MacropadError;

impl Actor for Macropad {
    type Args = (ActorRef<broker::Broker<crate::BrokerMessage>>,);
    type Error = MacropadError;

    async fn on_start(state: Self::Args, actor_ref: ActorRef<Self>) -> Result<Self, Self::Error> {
        let (broker_ref,) = state;

        let toggl_ref = crate::toggl::Toggl::spawn_link(&actor_ref, (broker_ref.clone(),)).await;

        #[cfg(feature = "pi")]
        {
            let serial_port =
                tokio_serial::new(std::env::var("MACROPAD_SERIAL_PATH").unwrap(), 19200)
                    .open_native_async()
                    .unwrap();
            tracing::info!("serial opened");

            let device = Framed::new(serial_port, LinesCodec::new());
            let (transmit, receive) = device.split::<String>();

            actor_ref.attach_stream(receive, (), ());

            let serial_sink = crate::serial_sink::SerialSink::new(transmit);

            Ok(Self {
                transmit: Box::new(serial_sink),
                broker_ref,
                toggl_ref,
            })
        }

        #[cfg(not(feature = "pi"))]
        {
            Ok(Self {
                transmit: Box::new(crate::serial_sink::DummySink),
                broker_ref,
                toggl_ref,
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

                if line.starts_with("p") {
                    return;
                }

                if line.starts_with("x") {
                    return;
                }

                if line.starts_with("y") {
                    return;
                }

                tracing::info!("<- {}", line);

                let message: serde_json::Value = serde_json::from_str(&line).unwrap();
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
            "readInbox" => self.read_inbox().await,
            "clearInbox" => self.clear_inbox().await,
            "startClock" => self.start_clock().await,
            _ => panic!("Unknown message kind: {}", kind),
        };

        match result {
            Ok(_) => self.send_success_message().await,
            Err(_) => self.send_error_message().await,
        }
    }

    async fn read_inbox(&mut self) -> Result<(), ()> {
        self.broker_ref
            .tell(broker::Publish {
                topic: "message".parse().unwrap(),
                message: crate::BrokerMessage::ReadInbox,
            })
            .await
            .map_err(|_| ())?;

        Ok(())
    }

    async fn clear_inbox(&mut self) -> Result<(), ()> {
        self.broker_ref
            .tell(broker::Publish {
                topic: "message".parse().unwrap(),
                message: crate::BrokerMessage::ClearInbox,
            })
            .await
            .map_err(|_| ())?;

        Ok(())
    }

    async fn start_clock(&mut self) -> Result<(), ()> {
        self.broker_ref
            .tell(broker::Publish {
                topic: "clock".parse().unwrap(),
                message: crate::BrokerMessage::StartClock,
            })
            .await
            .map_err(|_| ())?;

        Ok(())
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
        tracing::info!("-> {:?}", message);
        self.transmit.send(message.to_string()).await.unwrap();
    }
}
