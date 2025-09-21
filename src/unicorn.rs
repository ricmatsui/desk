use chrono::Timelike;
use kameo::error::Infallible;
use kameo::prelude::*;
use kameo_actors::broker;

pub struct Unicorn {
    client: reqwest::Client,
    base_url: reqwest::Url,
}

impl Actor for Unicorn {
    type Args = (ActorRef<broker::Broker<crate::BrokerMessage>>,);
    type Error = Infallible;

    async fn on_start(state: Self::Args, actor_ref: ActorRef<Self>) -> Result<Self, Self::Error> {
        let broker_ref = state.0;

        broker_ref
            .tell(broker::Subscribe {
                topic: "calendar".parse().unwrap(),
                recipient: actor_ref.clone().recipient(),
            })
            .await
            .unwrap();

        broker_ref
            .tell(broker::Subscribe {
                topic: "message".parse().unwrap(),
                recipient: actor_ref.clone().recipient(),
            })
            .await
            .unwrap();

        broker_ref
            .tell(broker::Subscribe {
                topic: "clock".parse().unwrap(),
                recipient: actor_ref.clone().recipient(),
            })
            .await
            .unwrap();

        Ok(Self {
            client: reqwest::Client::new(),
            base_url: reqwest::Url::parse(&std::env::var("UNICORN_BASE_URL").unwrap()).unwrap(),
        })
    }
}

impl Message<crate::BrokerMessage> for Unicorn {
    type Reply = ();

    async fn handle(
        &mut self,
        message: crate::BrokerMessage,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        match message {
            crate::BrokerMessage::CalendarEventUpcoming(event) => {
                tracing::info!("unicorn message: {:?}", event);

                let now = chrono::Local::now();

                let seconds = (event.start_at - now).num_seconds();

                self.client
                    .get(self.base_url.join("/countdown").unwrap())
                    .query(&[("seconds", seconds.to_string())])
                    .send()
                    .await
                    .unwrap()
                    .error_for_status()
                    .unwrap();
            }
            crate::BrokerMessage::Message(message) => {
                tracing::info!("unicorn message: {:?}", message);

                let text = any_ascii::any_ascii(&message.text);

                self.client
                    .post(self.base_url.join("/message").unwrap())
                    .json(&serde_json::json!({
                        "text": text,
                        "effects": message.effects,
                        "read": message.read,
                    }))
                    .send()
                    .await
                    .unwrap()
                    .error_for_status()
                    .unwrap();
            }
            crate::BrokerMessage::ReadInbox => {
                tracing::info!("read inbox");

                self.client
                    .get(self.base_url.join("/read-inbox").unwrap())
                    .send()
                    .await
                    .unwrap()
                    .error_for_status()
                    .unwrap();
            }
            crate::BrokerMessage::ClearInbox => {
                tracing::info!("clear inbox");

                self.client
                    .get(self.base_url.join("/clear-inbox").unwrap())
                    .send()
                    .await
                    .unwrap()
                    .error_for_status()
                    .unwrap();
            }
            crate::BrokerMessage::StartClock => {
                tracing::info!("start clock");

                let now = chrono::Local::now();

                self.client
                    .get(self.base_url.join("/start-clock").unwrap())
                    .query(&[(
                        "start_timestamp",
                        (now.hour() * 3600 + now.minute() * 60 + now.second()).to_string(),
                    )])
                    .send()
                    .await
                    .unwrap()
                    .error_for_status()
                    .unwrap();
            }
            _ => {}
        }
    }
}
