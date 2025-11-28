use axum::{Json, Router, http::StatusCode, routing::post};
use chrono::TimeZone;
use kameo::error::Infallible;
use kameo::prelude::*;

pub struct Urban {
    client: reqwest::Client,
    readings_queue: Vec<serde_json::Value>,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
}

impl Actor for Urban {
    type Args = ();
    type Error = Infallible;

    async fn on_start(_state: Self::Args, actor_ref: ActorRef<Self>) -> Result<Self, Self::Error> {
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

        let (shutdown, shutdown_receive) = tokio::sync::oneshot::channel();

        let submit_actor_ref = actor_ref.clone();

        tokio::spawn(async move {
            let app = Router::new().route(
                "/submit",
                post(
                    |axum::extract::Json(payload): Json<serde_json::Value>| async move {
                        submit_actor_ref
                            .tell(UrbanReadings {
                                payload: payload.clone(),
                            })
                            .await
                            .unwrap();
                        StatusCode::OK
                    },
                ),
            );

            let listener = tokio::net::TcpListener::bind("0.0.0.0:9002").await.unwrap();
            axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    shutdown_receive.await.unwrap();
                })
                .await
                .unwrap();
        });

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));

            loop {
                interval.tick().await;

                if actor_ref.tell(Tick).await.is_err() {
                    break;
                }
            }
        });

        Ok(Self {
            client,
            readings_queue: Vec::new(),
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

pub struct UrbanReadings {
    payload: serde_json::Value,
}

impl Message<UrbanReadings> for Urban {
    type Reply = ();

    async fn handle(
        &mut self,
        message: UrbanReadings,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        tracing::info!("readings {:?}", message.payload);
        // {
        //   "nickname": "",
        //   "timestamp": "2025-09-20 07:32:29",
        //   "readings": {
        //     "ec02": 400,
        //     "humidity": 51.23,
        //     "noise": 1.5,
        //     "pm1": 2,
        //     "pm10": 56,
        //     "pm2_5": 10,
        //     "pressure": 1016.36,
        //     "temperature": 25.95,
        //     "tvoc": 0
        //   }
        // }
        self.readings_queue.push(message.payload);
    }
}

pub struct Tick;

impl Message<Tick> for Urban {
    type Reply = ();

    async fn handle(
        &mut self,
        _message: Tick,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        if self.readings_queue.is_empty() {
            return;
        }

        let keys = self.readings_queue[0]["readings"]
            .as_object()
            .unwrap()
            .keys();

        let series = keys
            .map(|key| {
                let points = self
                    .readings_queue
                    .iter()
                    .map(move |readings| {
                        let timestamp = chrono::DateTime::parse_from_rfc3339(
                            readings["timestamp"].as_str().unwrap(),
                        )
                        .unwrap()
                        .with_timezone(&chrono::Utc)
                        .timestamp();

                        serde_json::json!({
                            "timestamp": timestamp,
                            "value": readings["readings"][key].as_f64().unwrap()
                        })
                    })
                    .collect::<Vec<serde_json::Value>>();

                serde_json::json!({
                    "metric": format!("urban.{}", key),
                    "type": 3,
                    "points": points,
                    "resources": [{ "name": "deskpi", "type": "host" }],
                })
            })
            .collect::<Vec<serde_json::Value>>();

        self.client
            .post("https://api.datadoghq.com/api/v2/series")
            .json(&serde_json::json!({ "series": series }))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();

        tracing::info!("submitted metrics: {:?}", series);

        self.readings_queue.clear();
    }
}
