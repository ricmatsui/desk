use kameo::error::Infallible;
use kameo::prelude::*;

use crate::apps::dashboard::{Dashboard, Monitor, Status};
use crate::backlight;
use crate::raylib_manager;
use crate::restarting_manager;

const MONITOR_QUERY: &str = "status:(alert OR warn OR \"no data\") muted:false";
const OFFLINE_FAILURES: u32 = 3;

pub struct Datadog {
    client: reqwest::Client,
    raylib_manager_ref: ActorRef<raylib_manager::RaylibManager>,
    backlight_manager_ref: ActorRef<restarting_manager::RestartingManager<backlight::Backlight>>,
    dashboard: Dashboard,
    display: Display,
    backlight_requested: bool,
    failures: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Display {
    Off,
    /// Failing monitors or offline
    Failing,
    AllClear,
}

impl Actor for Datadog {
    type Args = (
        ActorRef<raylib_manager::RaylibManager>,
        ActorRef<restarting_manager::RestartingManager<backlight::Backlight>>,
    );
    type Error = Infallible;

    fn prepare() -> PreparedActor<Self> {
        Self::prepare_with_mailbox(mailbox::unbounded())
    }

    async fn on_start(state: Self::Args, actor_ref: ActorRef<Self>) -> Result<Self, Self::Error> {
        let (raylib_manager_ref, backlight_manager_ref) = state;

        let mut headers = reqwest::header::HeaderMap::new();

        headers.insert(
            "DD-API-KEY",
            reqwest::header::HeaderValue::from_str(&std::env::var("DATADOG_API_KEY").unwrap())
                .unwrap(),
        );
        headers.insert(
            "DD-APPLICATION-KEY",
            reqwest::header::HeaderValue::from_str(&std::env::var("DATADOG_APP_KEY").unwrap())
                .unwrap(),
        );

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap();

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
            raylib_manager_ref,
            backlight_manager_ref,
            dashboard: Dashboard::default(),
            display: Display::Off,
            backlight_requested: false,
            failures: 0,
        })
    }

    async fn on_stop(
        &mut self,
        _actor_ref: WeakActorRef<Self>,
        _reason: ActorStopReason,
    ) -> Result<(), Self::Error> {
        let _ = self.update_display(Display::Off).await;
        Ok(())
    }
}

pub struct Tick;

impl Message<Tick> for Datadog {
    type Reply = ();

    async fn handle(
        &mut self,
        _message: Tick,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        match self.fetch_monitors().await {
            Ok(monitors) => {
                self.dashboard = Dashboard {
                    monitors,
                    polled_at: Some(chrono::Local::now()),
                    offline: None,
                };
                self.failures = 0;
            }
            Err(error) => {
                tracing::warn!("failed to fetch monitors: {}", error);
                self.failures += 1;

                // Ride out brief blips, the current display stays up meanwhile
                if self.failures < OFFLINE_FAILURES {
                    return;
                }

                // Keep the last monitors and poll time, shown as stale
                self.dashboard.offline = Some(offline_reason(&error));
            }
        }

        // Failing monitors or offline, then all clear for one poll, then off
        let display = match (self.dashboard.is_clear(), self.display) {
            (false, _) => Display::Failing,
            (true, Display::Failing) => Display::AllClear,
            (true, Display::AllClear | Display::Off) => Display::Off,
        };

        if self.update_display(display).await.is_err() {
            tracing::warn!("failed to update display");
        }
    }
}

impl Datadog {
    /// Draws the dashboard for the display and requests or releases the backlight to match,
    /// keeping requests and releases balanced
    async fn update_display(&mut self, display: Display) -> Result<(), ()> {
        if display == Display::Off && self.display == Display::Off {
            return Ok(());
        }

        self.display = display;

        let enabled = display != Display::Off;

        // Draw before turning on and blank before turning off, so stale frames are never lit
        self.raylib_manager_ref
            .tell(raylib_manager::ShowDashboard(
                enabled.then(|| self.dashboard.clone()),
            ))
            .await
            .map_err(|_| ())?;

        if enabled != self.backlight_requested {
            let backlight_ref = self
                .backlight_manager_ref
                .ask(restarting_manager::GetChild)
                .await
                .map_err(|_| ())?
                .ok_or(())?;

            if enabled {
                backlight_ref.tell(backlight::Request).await.map_err(|_| ())?;
            } else {
                backlight_ref.tell(backlight::Release).await.map_err(|_| ())?;
            }

            self.backlight_requested = enabled;
        }

        Ok(())
    }

    async fn fetch_monitors(&self) -> Result<Vec<Monitor>, reqwest::Error> {
        let response = self
            .client
            .get("https://api.datadoghq.com/api/v1/monitor/search")
            .query(&[("query", MONITOR_QUERY), ("per_page", "100")])
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await?;

        let mut monitors = response["monitors"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|monitor| {
                let status = match monitor["status"].as_str()? {
                    "Alert" => Status::Alert,
                    "Warn" => Status::Warn,
                    "No Data" => Status::NoData,
                    _ => return None,
                };

                let triggered_at = monitor["last_triggered_ts"]
                    .as_i64()
                    .and_then(|timestamp| chrono::DateTime::from_timestamp(timestamp, 0))
                    .map(|timestamp| timestamp.with_timezone(&chrono::Local));

                Some(Monitor {
                    name: display_name(monitor["name"].as_str().unwrap_or_default()),
                    status,
                    triggered_at,
                })
            })
            .collect::<Vec<_>>();

        // Worst status first, then most recently triggered
        monitors.sort_by(|a, b| {
            a.status
                .cmp(&b.status)
                .then(b.triggered_at.cmp(&a.triggered_at))
        });

        Ok(monitors)
    }
}

/// Removes template variables and non-ASCII characters the default font cannot draw
fn display_name(name: &str) -> String {
    let mut result = String::new();
    let mut rest = name;

    while let Some(start) = rest.find("{{") {
        result.push_str(&rest[..start]);
        rest = match rest[start..].find("}}") {
            Some(end) => &rest[start + end + 2..],
            None => "",
        };
    }
    result.push_str(rest);

    any_ascii::any_ascii(&result)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn offline_reason(error: &reqwest::Error) -> String {
    if let Some(status) = error.status() {
        format!("HTTP {}", status.as_u16())
    } else if error.is_timeout() {
        "Timed out".to_string()
    } else if error.is_connect() {
        "No connection".to_string()
    } else if error.is_decode() {
        "Bad response".to_string()
    } else {
        "Request failed".to_string()
    }
}
