use base64::prelude::*;
use kameo::error::Infallible;
use kameo::message::StreamMessage;
use kameo::prelude::*;
use kameo_actors::broker;

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

pub struct ThinkInk {
    transmit: Box<dyn Transmitter>,
    raylib_actor_ref: ActorRef<crate::raylib_actor::Raylib>,
    last_date_string: Option<String>,
}

impl Actor for ThinkInk {
    type Args = (
        ActorRef<broker::Broker<crate::BrokerMessage>>,
        ActorRef<crate::raylib_actor::Raylib>,
    );
    type Error = Infallible;

    async fn on_start(state: Self::Args, actor_ref: ActorRef<Self>) -> Result<Self, Self::Error> {
        let broker_ref = state.0;
        let raylib_actor_ref = state.1;

        broker_ref
            .tell(broker::Subscribe {
                topic: "*".parse().unwrap(),
                recipient: actor_ref.clone().recipient(),
            })
            .await
            .unwrap();

        crate::light::Light::spawn_link(&actor_ref, (actor_ref.clone(),)).await;

        actor_ref.tell(UpdateImage).try_send().unwrap();

        let last_date_string = match tokio::fs::read_to_string("date.txt").await {
            Ok(data) => Some(data),
            Err(_) => None,
        };

        #[cfg(feature = "pi")]
        {
            let serial_port =
                tokio_serial::new(std::env::var("THINKINK_SERIAL_PATH").unwrap(), 115200)
                    .open_native_async()
                    .unwrap();
            tracing::info!("serial opened");

            let device = Framed::new(serial_port, LinesCodec::new());
            let (transmit, receive) = device.split::<String>();

            actor_ref.attach_stream(receive, (), ());

            let serial_transmitter = SerialTransmitter { inner: transmit };

            Ok(Self {
                transmit: Box::new(serial_transmitter),
                raylib_actor_ref,
                last_date_string,
            })
        }

        #[cfg(not(feature = "pi"))]
        {
            Ok(Self {
                transmit: Box::new(DummyTransmitter),
                raylib_actor_ref,
                last_date_string,
            })
        }
    }
}

impl Message<StreamMessage<Result<String, tokio_util::codec::LinesCodecError>, (), ()>>
    for ThinkInk
{
    type Reply = ();

    async fn handle(
        &mut self,
        message: StreamMessage<Result<String, tokio_util::codec::LinesCodecError>, (), ()>,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        match message {
            StreamMessage::Next(Ok(line)) => {
                if line == "t" {
                    return;
                }

                let message: serde_json::Value = serde_json::from_str(&line).unwrap();

                tracing::info!("<- message: {:?}", message);
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

impl Message<crate::BrokerMessage> for ThinkInk {
    type Reply = ();

    async fn handle(
        &mut self,
        message: crate::BrokerMessage,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        match message {
            crate::BrokerMessage::TimeEntryStarted(_) => {
                self.send_message(serde_json::json!({
                    "kind": "startAnimation",
                }))
                .await
            }
            crate::BrokerMessage::TimeEntryStopped => {
                self.send_message(serde_json::json!({
                    "kind": "stopAnimation",
                }))
                .await;
            }
            crate::BrokerMessage::TimeEntryTimeUpdated(update) => {
                self.send_message(serde_json::json!({
                    "kind": "adjustAnimationTime",
                    "minutes": update.minutes,
                }))
                .await;
            }
        }
    }
}

pub struct UpdateLight {
    pub target_value: u32,
    pub speed: u32,
}

impl Message<UpdateLight> for ThinkInk {
    type Reply = ();

    async fn handle(
        &mut self,
        message: UpdateLight,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.send_message(serde_json::json!({
            "kind": "light",
            "targetValue": message.target_value,
            "speed": message.speed,
        }))
        .await;
    }
}

pub struct UpdateImage;

impl Message<UpdateImage> for ThinkInk {
    type Reply = ();

    async fn handle(
        &mut self,
        _message: UpdateImage,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        let now = chrono::Local::now();

        let current_date_string = format!("{}", now.format("%m-%d"));

        if let Some(last_date_string) = &self.last_date_string {
            if *last_date_string == current_date_string {
                tracing::info!("skipping image update");
                return;
            }
        }

        let data = self
            .raylib_actor_ref
            .ask(crate::raylib_actor::RenderThinkInkImage)
            .await
            .unwrap();

        for (index, chunk_data) in data.as_slice().chunks(256).enumerate() {
            self.send_message(serde_json::json!({
                "kind": "displayData",
                "offset": index * 256,
                "data": BASE64_STANDARD.encode(chunk_data),
            }))
            .await;
        }

        self.send_message(serde_json::json!({ "kind": "refreshDisplay", }))
            .await;

        self.last_date_string = Some(current_date_string);
        tokio::fs::write("date.txt", self.last_date_string.as_ref().unwrap())
            .await
            .unwrap();
    }
}

impl ThinkInk {
    async fn send_message(&mut self, message: serde_json::Value) {
        tracing::info!("-> message: {:?}", message);
        self.transmit.send(message.to_string()).await.unwrap();
    }
}
