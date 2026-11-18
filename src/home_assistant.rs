use axum::{Json, Router, http::StatusCode, routing::post};
use kameo::error::Infallible;
use kameo::prelude::*;
use kameo_actors::broker;
use std::io::ErrorKind;

pub struct HomeAssistant {
    broker_ref: ActorRef<broker::Broker<crate::BrokerMessage>>,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
}

impl Actor for HomeAssistant {
    type Args = (ActorRef<broker::Broker<crate::BrokerMessage>>,);
    type Error = Infallible;

    fn prepare() -> PreparedActor<Self> {
        Self::prepare_with_mailbox(mailbox::unbounded())
    }

    async fn on_start(state: Self::Args, actor_ref: ActorRef<Self>) -> Result<Self, Self::Error> {
        let broker_ref = state.0;

        let (shutdown, shutdown_receive) = tokio::sync::oneshot::channel();

        let calendar_actor_ref = actor_ref.clone();
        let message_actor_ref = actor_ref.clone();
        let heartbeat_actor_ref = actor_ref.clone();

        // Bound before returning so the restarted actor owns the port once the previous one's server lets go of it
        let listener = bind_listener().await;

        tokio::spawn(async move {
            let app = Router::new()
                .route(
                    "/calendar",
                    post(
                        |axum::extract::Json(payload): Json<serde_json::Value>| async move {
                            calendar_actor_ref
                                .tell(HomeAssistantUpdate {
                                    payload: payload.clone(),
                                })
                                .await
                                .unwrap();
                            StatusCode::OK
                        },
                    ),
                )
                .route(
                    "/message",
                    post(
                        |axum::extract::Json(payload): Json<serde_json::Value>| async move {
                            message_actor_ref
                                .tell(HomeAssistantMessage {
                                    payload: payload.clone(),
                                })
                                .await
                                .unwrap();
                            StatusCode::OK
                        },
                    ),
                )
                .route(
                    "/heartbeat",
                    post(|| async move {
                        heartbeat_actor_ref.tell(HomeAssistantHeartbeat).await.unwrap();
                        StatusCode::OK
                    }),
                );

            axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    shutdown_receive.await.unwrap();
                })
                .await
                .unwrap();
        });

        Ok(Self {
            broker_ref,
            shutdown: Some(shutdown),
        })
    }

    async fn on_stop(
        &mut self,
        _actor_ref: WeakActorRef<Self>,
        _reason: ActorStopReason,
    ) -> Result<(), Self::Error> {
        if let Some(shutdown) = self.shutdown.take() {
            shutdown.send(()).unwrap();
        }
        Ok(())
    }
}

pub struct HomeAssistantUpdate {
    payload: serde_json::Value,
}

impl Message<HomeAssistantUpdate> for HomeAssistant {
    type Reply = ();

    async fn handle(
        &mut self,
        message: HomeAssistantUpdate,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        // {
        //   "uid": "abc@example.com"
        //   "summary": "",
        //   "all_day": false,
        //   "start": "2025-09-02T23:30:00-05:00",
        //   "end": "2025-09-02T23:55:00-05:00",
        //   "location": "",
        //   "description": "",
        // }
        //

        tracing::info!("update {:?}", message.payload);

        if message.payload["all_day"].as_bool().unwrap() {
            return;
        }

        self.broker_ref
            .tell(broker::Publish {
                topic: "calendar".parse().unwrap(),
                message: crate::BrokerMessage::CalendarEventUpcoming(
                    crate::CalendarEventUpcoming {
                        description: message.payload["summary"].as_str().unwrap().to_string(),

                        start_at: chrono::DateTime::<chrono::FixedOffset>::parse_from_rfc3339(
                            message.payload["start"].as_str().unwrap(),
                        )
                        .unwrap()
                        .with_timezone(&chrono::Local),

                        end_at: chrono::DateTime::<chrono::FixedOffset>::parse_from_rfc3339(
                            message.payload["end"].as_str().unwrap(),
                        )
                        .unwrap()
                        .with_timezone(&chrono::Local),
                    },
                ),
            })
            .await
            .unwrap();
    }
}

pub struct HomeAssistantMessage {
    payload: serde_json::Value,
}

impl Message<HomeAssistantMessage> for HomeAssistant {
    type Reply = ();

    async fn handle(
        &mut self,
        message: HomeAssistantMessage,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        // {
        //   "text": "Hello World!",
        //   "effects": ["rainbow"]
        //   "read": false,
        //   "icon": "calendar",
        //   "ttl": 3600,
        //   "id": "datadog",
        //   "sound": "meow"
        // }
        tracing::info!("message {:?}", message.payload);

        self.broker_ref
            .tell(broker::Publish {
                topic: "message".parse().unwrap(),
                message: crate::BrokerMessage::Message(crate::Message {
                    text: message.payload["text"].as_str().unwrap().to_string(),
                    effects: message.payload["effects"]
                        .as_array()
                        .map(|effects| {
                            effects
                                .iter()
                                .map(|e| e.as_str().unwrap().to_string())
                                .collect()
                        })
                        .unwrap_or_default(),
                    read: message.payload["read"].as_bool().unwrap_or(false),
                    icon: message.payload["icon"].as_str().map(|i| i.to_string()),
                    ttl: message.payload["ttl"].as_u64(),
                    id: message.payload["id"].as_str().map(|i| i.to_string()),
                    sound: message.payload["sound"].as_str().map(|s| s.to_string()),
                }),
            })
            .await
            .unwrap();
    }
}

pub struct HomeAssistantHeartbeat;

impl Message<HomeAssistantHeartbeat> for HomeAssistant {
    type Reply = ();

    async fn handle(
        &mut self,
        _: HomeAssistantHeartbeat,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.broker_ref
            .tell(broker::Publish {
                topic: "heartbeat".parse().unwrap(),
                message: crate::BrokerMessage::Heartbeat,
            })
            .await
            .unwrap();
    }
}

// The previous actor's server drains its connections after a restart, so the port can stay taken for a moment.
// Past BIND_ATTEMPTS the panic stops the actor and RestartingManager takes over.
const BIND_ATTEMPTS: u32 = 10;

async fn bind_listener() -> tokio::net::TcpListener {
    let mut attempt = 1;

    loop {
        match tokio::net::TcpListener::bind("0.0.0.0:9001").await {
            Ok(listener) => return listener,
            Err(error) if error.kind() == ErrorKind::AddrInUse && attempt < BIND_ATTEMPTS => {
                tracing::warn!("port 9001 still in use, retrying ({attempt}/{BIND_ATTEMPTS})");
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                attempt += 1;
            }
            Err(error) => panic!("failed to bind port 9001 ({attempt}/{BIND_ATTEMPTS}): {error}"),
        }
    }
}
