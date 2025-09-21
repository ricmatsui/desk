use base64::prelude::*;
use kameo::error::Infallible;
use kameo::prelude::*;
use kameo_actors::broker;

pub struct Toggl {
    client: reqwest::Client,
    base_url: reqwest::Url,
    workspace_id: i64,
    project_id: i64,
    broker_ref: ActorRef<broker::Broker<crate::BrokerMessage>>,
    current_time_entry: Option<serde_json::Value>,
}

#[derive(Debug)]
pub struct TogglError;

impl Actor for Toggl {
    type Args = (ActorRef<broker::Broker<crate::BrokerMessage>>,);
    type Error = Infallible;

    async fn on_start(state: Self::Args, actor_ref: ActorRef<Self>) -> Result<Self, Self::Error> {
        let mut headers = reqwest::header::HeaderMap::new();

        let mut authorization = reqwest::header::HeaderValue::from_str(&format!(
            "Basic {}",
            BASE64_STANDARD.encode(std::env::var("TOGGL_AUTH").unwrap())
        ))
        .unwrap();

        authorization.set_sensitive(true);

        headers.insert(reqwest::header::AUTHORIZATION, authorization);

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .unwrap();

        actor_ref.tell(GetCurrentTimeEntry).try_send().unwrap();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));

            loop {
                interval.tick().await;

                if actor_ref.tell(GetCurrentTimeEntry).await.is_err() {
                    break;
                }
            }
        });

        Ok(Self {
            client,
            base_url: reqwest::Url::parse("https://api.track.toggl.com").unwrap(),
            workspace_id: i64::from_str_radix(&std::env::var("TOGGL_WORKSPACE_ID").unwrap(), 10)
                .unwrap(),
            project_id: i64::from_str_radix(&std::env::var("TOGGL_PROJECT_ID").unwrap(), 10)
                .unwrap(),
            broker_ref: state.0,
            current_time_entry: None,
        })
    }
}

pub struct GetTimeEntries;

impl Message<GetTimeEntries> for Toggl {
    type Reply = Result<serde_json::Value, reqwest::Error>;

    async fn handle(
        &mut self,
        _message: GetTimeEntries,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        let result = self
            .client
            .get(self.base_url.join("/api/v9/me/time_entries").unwrap())
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await?;

        Ok(result)
    }
}

pub struct StartTimeEntry {
    pub description: String,
}

impl Message<StartTimeEntry> for Toggl {
    type Reply = Result<(), TogglError>;

    async fn handle(
        &mut self,
        message: StartTimeEntry,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        let time_entry = self
            .client
            .post(
                self.base_url
                    .join(&format!(
                        "/api/v9/workspaces/{}/time_entries",
                        &self.workspace_id
                    ))
                    .unwrap(),
            )
            .json(&serde_json::json!({
                "created_with": "desk",
                "project_id": self.project_id,
                "workspace_id": self.workspace_id,
                "start": chrono::Utc::now().to_rfc3339(),
                "duration": -1,
                "description": message.description,
            }))
            .send()
            .await
            .map_err(|_| TogglError)?
            .error_for_status()
            .map_err(|_| TogglError)?
            .json::<serde_json::Value>()
            .await
            .map_err(|_| TogglError)?;

        self.current_time_entry = Some(time_entry.clone());

        self.broker_ref
            .tell(broker::Publish {
                topic: "toggl".parse().unwrap(),
                message: crate::BrokerMessage::TimeEntryStarted(crate::TimeEntryStarted {
                    description: message.description,
                }),
            })
            .await
            .map_err(|_| TogglError)?;

        Ok(())
    }
}

pub struct StopTimeEntry;

impl Message<StopTimeEntry> for Toggl {
    type Reply = Result<(), TogglError>;

    async fn handle(
        &mut self,
        _message: StopTimeEntry,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        let current_time_entry = self
            .get_current_time_entry()
            .await
            .map_err(|_| TogglError)?;

        if current_time_entry.is_null() {
            self.current_time_entry = None;
            return Ok(());
        }

        self.client
            .patch(
                self.base_url
                    .join(&format!(
                        "/api/v9/workspaces/{}/time_entries/{}/stop",
                        &self.workspace_id,
                        current_time_entry["id"].as_i64().unwrap()
                    ))
                    .unwrap(),
            )
            .json(&serde_json::json!({}))
            .send()
            .await
            .map_err(|_| TogglError)?
            .error_for_status()
            .map_err(|_| TogglError)?;

        self.current_time_entry = None;

        self.broker_ref
            .tell(broker::Publish {
                topic: "toggl".parse().unwrap(),
                message: crate::BrokerMessage::TimeEntryStopped,
            })
            .await
            .map_err(|_| TogglError)?;

        Ok(())
    }
}

pub struct ContinueTimeEntry;

impl Message<ContinueTimeEntry> for Toggl {
    type Reply = Result<(), TogglError>;

    async fn handle(
        &mut self,
        _message: ContinueTimeEntry,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        let result = self
            .client
            .get(self.base_url.join("/api/v9/me/time_entries").unwrap())
            .send()
            .await
            .map_err(|_| TogglError)?
            .error_for_status()
            .map_err(|_| TogglError)?
            .json::<serde_json::Value>()
            .await
            .map_err(|_| TogglError)?;

        let entry = result.as_array().unwrap().first().unwrap();

        let description = entry["description"].as_str().unwrap();

        let time_entry = self.client
            .post(
                self.base_url
                    .join(&format!(
                        "/api/v9/workspaces/{}/time_entries",
                        &self.workspace_id
                    ))
                    .unwrap(),
            )
            .json(&serde_json::json!({
                "created_with": "desk",
                "project_id": self.project_id,
                "workspace_id": self.workspace_id,
                "start": chrono::Utc::now().to_rfc3339(),
                "duration": -1,
                "description": description,
            }))
            .send()
            .await
            .map_err(|_| TogglError)?
            .error_for_status()
            .map_err(|_| TogglError)?
            .json::<serde_json::Value>()
            .await
            .map_err(|_| TogglError)?;

        self.current_time_entry = Some(time_entry.clone());

        self.broker_ref
            .tell(broker::Publish {
                topic: "toggl".parse().unwrap(),
                message: crate::BrokerMessage::TimeEntryStarted(crate::TimeEntryStarted {
                    description: description.to_string(),
                }),
            })
            .await
            .map_err(|_| TogglError)?;

        Ok(())
    }
}

pub struct AdjustTime {
    pub minutes: i64,
}

#[derive(Debug)]
pub enum AdjustTimeError {
    NotFound,
    RequestError,
}

impl Message<AdjustTime> for Toggl {
    type Reply = Result<(), AdjustTimeError>;

    async fn handle(
        &mut self,
        message: AdjustTime,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        let current_time_entry = self
            .get_current_time_entry()
            .await
            .map_err(|_error| AdjustTimeError::RequestError)?;

        if current_time_entry.is_null() {
            return Err(AdjustTimeError::NotFound);
        }

        let current_start =
            chrono::DateTime::parse_from_rfc3339(current_time_entry["start"].as_str().unwrap())
                .unwrap();
        let updated_start = current_start - chrono::Duration::minutes(message.minutes);

        let time_entry = self.client
            .put(
                self.base_url
                    .join(&format!(
                        "/api/v9/workspaces/{}/time_entries/{}",
                        &self.workspace_id,
                        current_time_entry["id"].as_i64().unwrap()
                    ))
                    .unwrap(),
            )
            .json(&serde_json::json!({
                "start": updated_start.to_rfc3339()
            }))
            .send()
            .await
            .map_err(|_error| AdjustTimeError::RequestError)?
            .error_for_status()
            .map_err(|_error| AdjustTimeError::RequestError)?
            .json::<serde_json::Value>()
            .await
            .map_err(|_error| AdjustTimeError::RequestError)?;

        self.current_time_entry = Some(time_entry.clone());

        self.broker_ref
            .tell(broker::Publish {
                topic: "toggl".parse().unwrap(),
                message: crate::BrokerMessage::TimeEntryTimeUpdated(crate::TimeEntryTimeUpdated {
                    minutes: message.minutes,
                }),
            })
            .await
            .map_err(|_| AdjustTimeError::RequestError)?;

        Ok(())
    }
}

pub struct GetCurrentTimeEntry;

impl Message<GetCurrentTimeEntry> for Toggl {
    type Reply = Result<serde_json::Value, TogglError>;

    async fn handle(
        &mut self,
        _message: GetCurrentTimeEntry,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        let new_time_entry = self
            .get_current_time_entry()
            .await
            .map_err(|_| TogglError)?;

        let current_date = chrono::Utc::now().fixed_offset();

        if self.current_time_entry.is_none() {
            if new_time_entry.is_null() {
                // Nothing to do
            } else {
                self.broker_ref
                    .tell(broker::Publish {
                        topic: "toggl".parse().unwrap(),
                        message: crate::BrokerMessage::TimeEntryStarted(crate::TimeEntryStarted {
                            description: new_time_entry["description"]
                                .as_str()
                                .unwrap()
                                .to_string(),
                        }),
                    })
                    .await
                    .map_err(|_| TogglError)?;

                let new_start =
                    chrono::DateTime::parse_from_rfc3339(new_time_entry["start"].as_str().unwrap())
                        .unwrap();

                self.broker_ref
                    .tell(broker::Publish {
                        topic: "toggl".parse().unwrap(),
                        message: crate::BrokerMessage::TimeEntryTimeUpdated(
                            crate::TimeEntryTimeUpdated {
                                minutes: (current_date - new_start).num_minutes() as i64,
                            },
                        ),
                    })
                    .await
                    .map_err(|_| TogglError)?;
            }
        } else {
            if new_time_entry.is_null() {
                self.broker_ref
                    .tell(broker::Publish {
                        topic: "toggl".parse().unwrap(),
                        message: crate::BrokerMessage::TimeEntryStopped,
                    })
                    .await
                    .map_err(|_| TogglError)?;
            } else {
                let current_time_entry = self.current_time_entry.as_ref().unwrap();

                let current_start = chrono::DateTime::parse_from_rfc3339(
                    current_time_entry["start"].as_str().unwrap(),
                )
                .unwrap();

                let new_start =
                    chrono::DateTime::parse_from_rfc3339(new_time_entry["start"].as_str().unwrap())
                        .unwrap();

                self.broker_ref
                    .tell(broker::Publish {
                        topic: "toggl".parse().unwrap(),
                        message: crate::BrokerMessage::TimeEntryTimeUpdated(
                            crate::TimeEntryTimeUpdated {
                                minutes: (current_start - new_start).num_minutes() as i64,
                            },
                        ),
                    })
                    .await
                    .map_err(|_| TogglError)?;
            }
        }

        if new_time_entry.is_null() {
            self.current_time_entry = None;
        } else {
            self.current_time_entry = Some(new_time_entry.clone());
        }

        Ok(new_time_entry)
    }
}

impl Toggl {
    async fn get_current_time_entry(&self) -> Result<serde_json::Value, reqwest::Error> {
        let result = self
            .client
            .get(
                self.base_url
                    .join("/api/v9/me/time_entries/current")
                    .unwrap(),
            )
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await?;

        Ok(result)
    }
}
