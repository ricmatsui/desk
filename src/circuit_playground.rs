use kameo::error::Infallible;
use kameo::message::StreamMessage;
use kameo::prelude::*;

#[cfg(feature = "pi")]
use futures::stream::{StreamExt};
#[cfg(feature = "pi")]
use tokio_serial::SerialPortBuilderExt;
#[cfg(feature = "pi")]
use tokio_util::codec::{Framed, LinesCodec};

pub struct CircuitPlayground {
    client: reqwest::Client,
    last_metrics_submitted_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl Actor for CircuitPlayground {
    type Args = ();
    type Error = Infallible;

    async fn on_start(_state: Self::Args, _actor_ref: ActorRef<Self>) -> Result<Self, Self::Error> {
        let mut headers = reqwest::header::HeaderMap::new();

        headers.insert(
            "DD-API-KEY",
            reqwest::header::HeaderValue::from_str(&std::env::var("DATADOG_API_KEY").unwrap())
                .unwrap(),
        );

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .unwrap();

        #[cfg(feature = "pi")]
        {
            let serial_port = tokio_serial::new(
                std::env::var("CIRCUIT_PLAYGROUND_SERIAL_PATH").unwrap(),
                115200,
            )
            .open_native_async()
            .unwrap();
            tracing::info!("serial opened");

            let device = Framed::new(serial_port, LinesCodec::new());
            let (_transmit, receive) = device.split::<String>();

            _actor_ref.attach_stream(receive, (), ());

            Ok(Self {
                last_metrics_submitted_at: None,
                client,
            })
        }

        #[cfg(not(feature = "pi"))]
        {
            Ok(Self {
                last_metrics_submitted_at: None,
                client,
            })
        }
    }
}

impl Message<StreamMessage<Result<String, tokio_util::codec::LinesCodecError>, (), ()>>
    for CircuitPlayground
{
    type Reply = ();

    async fn handle(
        &mut self,
        message: StreamMessage<Result<String, tokio_util::codec::LinesCodecError>, (), ()>,
        context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        match message {
            StreamMessage::Next(Ok(line)) => {
                if line == "c" {
                    return;
                }

                if line.starts_with("ra1") {
                    let value = line["ra1".len()..].parse::<u32>().unwrap();

                    context
                        .actor_ref()
                        .tell(ReadingUpdated { value })
                        .await
                        .unwrap();
                }
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

struct ReadingUpdated {
    value: u32,
}

impl Message<ReadingUpdated> for CircuitPlayground {
    type Reply = ();

    async fn handle(
        &mut self,
        message: ReadingUpdated,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        let value = message.value;

        let current_date = chrono::Utc::now();

        if let Some(last_metrics_submitted_at) = self.last_metrics_submitted_at
            && current_date - last_metrics_submitted_at < chrono::Duration::minutes(1)
        {
            return;
        }

        self.client
            .post("https://api.datadoghq.com/api/v2/series")
            .json(&serde_json::json!({
                "series": [
                    {
                        "metric": "soil.capacitance",
                        "type": 3,
                        "points": [{ "timestamp": current_date.timestamp(), "value": value, }],
                        "resources": [{ "name": "deskpi", "type": "host" }],
                        "tags": ["input:a1"],
                    }
                ]
            }))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();

        tracing::info!("submitted metrics: {}", value);

        self.last_metrics_submitted_at = Some(current_date);
    }
}
