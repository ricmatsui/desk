use chrono::Timelike;
use kameo::error::Infallible;
use kameo::prelude::*;
use kameo_actors::broker;

pub struct Unicorn {
    client: reqwest::Client,
    base_url: reqwest::Url,
    schedule: Vec<ScheduledItem>,
    wake_task: Option<tokio::task::JoinHandle<()>>,
}

impl Actor for Unicorn {
    type Args = (ActorRef<broker::Broker<crate::BrokerMessage>>,);
    type Error = Infallible;

    fn prepare() -> PreparedActor<Self> {
        Self::prepare_with_mailbox(mailbox::unbounded())
    }

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

        broker_ref
            .tell(broker::Subscribe {
                topic: "countdown".parse().unwrap(),
                recipient: actor_ref.clone().recipient(),
            })
            .await
            .unwrap();

        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap(),
            base_url: reqwest::Url::parse(&std::env::var("UNICORN_BASE_URL").unwrap()).unwrap(),
            schedule: Vec::new(),
            wake_task: None,
        })
    }
}

impl Message<crate::BrokerMessage> for Unicorn {
    type Reply = ();

    async fn handle(
        &mut self,
        message: crate::BrokerMessage,
        context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        match message {
            crate::BrokerMessage::CalendarEventUpcoming(event) => {
                tracing::info!("unicorn message: {:?}", event);

                let now = chrono::Local::now();
                add_calendar_event(&mut self.schedule, event.start_at, event.end_at, now);
                self.process_schedule(context.actor_ref()).await;
            }
            crate::BrokerMessage::StartCountdown(minutes) => {
                tracing::info!("unicorn message: {:?}", minutes);

                self.client
                    .get(self.base_url.join("/countdown").unwrap())
                    .query(&[
                        ("seconds", (minutes * 60).to_string()),
                        ("icon", "timer".to_string()),
                    ])
                    .send()
                    .await
                    .unwrap()
                    .error_for_status()
                    .unwrap();
            }

            crate::BrokerMessage::CancelAnimation => {
                tracing::info!("unicorn: stopping countdown");

                self.client
                    .get(self.base_url.join("/cancel-animation").unwrap())
                    .send()
                    .await
                    .unwrap()
                    .error_for_status()
                    .unwrap();
            }
            crate::BrokerMessage::StartTimestampCountdown(timestamp) => {
                tracing::info!("unicorn message: {:?}", timestamp);

                self.client
                    .get(self.base_url.join("/countdown").unwrap())
                    .query(&[
                        ("timestamp", timestamp.to_string()),
                        ("icon", "timer".to_string()),
                    ])
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

#[derive(Debug)]
pub struct WakeUp;

impl Message<WakeUp> for Unicorn {
    type Reply = ();

    async fn handle(
        &mut self,
        _: WakeUp,
        context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.process_schedule(context.actor_ref()).await;
    }
}

impl Unicorn {
    async fn process_schedule(&mut self, actor_ref: ActorRef<Self>) {
        let now = chrono::Local::now();
        let due = pop_due(&mut self.schedule, now);
        for item in due {
            self.send_action(&item.action).await;
        }

        if let Some(handle) = self.wake_task.take() {
            handle.abort();
        }

        if let Some(next) = self.schedule.first() {
            let next_at = next.at;

            let handle = tokio::spawn(async move {
                let now = chrono::Local::now();
                let sleep_duration = (next_at - now)
                    .to_std()
                    .unwrap_or(std::time::Duration::ZERO);
                tokio::time::sleep(sleep_duration).await;
                let _ = actor_ref.tell(WakeUp).await;
            });
            self.wake_task = Some(handle);
        }
    }

    async fn send_action(&self, action: &ScheduledAction) {
        let (icon, target) = match action {
            ScheduledAction::SendCountdown { target } => ("calendar", target),
            ScheduledAction::SendEnding { target } => ("flag", target),
        };

        tracing::info!("unicorn schedule: send {} to {:?}", icon, target);
        self.client
            .get(self.base_url.join("/countdown").unwrap())
            .query(&[
                ("timestamp", target.timestamp().to_string()),
                ("icon", icon.to_string()),
            ])
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ScheduledAction {
    SendCountdown { target: chrono::DateTime<chrono::Local> },
    SendEnding { target: chrono::DateTime<chrono::Local> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ScheduledItem {
    at: chrono::DateTime<chrono::Local>,
    action: ScheduledAction,
}

fn action_rank(action: &ScheduledAction) -> u8 {
    match action {
        ScheduledAction::SendEnding { .. } => 0,
        ScheduledAction::SendCountdown { .. } => 1,
    }
}

fn sort_schedule(schedule: &mut Vec<ScheduledItem>) {
    schedule.sort_by_key(|item| (item.at, action_rank(&item.action)));
}

fn add_calendar_event(
    schedule: &mut Vec<ScheduledItem>,
    start_at: chrono::DateTime<chrono::Local>,
    end_at: chrono::DateTime<chrono::Local>,
    now: chrono::DateTime<chrono::Local>,
) {
    let chained_idx = schedule.iter().position(|item| {
        if let ScheduledAction::SendEnding { target } = &item.action {
            *target == start_at
        } else {
            false
        }
    });

    let overlaps_prior_event = schedule.iter().any(|item| {
        matches!(&item.action, ScheduledAction::SendEnding { target } if *target > start_at)
    });

    let countdown_at = if let Some(idx) = chained_idx {
        schedule.remove(idx);
        now
    } else if overlaps_prior_event {
        now
    } else {
        let latest_ending_at = schedule
            .iter()
            .filter_map(|item| match item.action {
                ScheduledAction::SendEnding { .. } => Some(item.at),
                _ => None,
            })
            .max();
        latest_ending_at.unwrap_or(now)
    };

    schedule.push(ScheduledItem {
        at: countdown_at,
        action: ScheduledAction::SendCountdown { target: start_at },
    });

    let ending_at = std::cmp::max(
        now,
        std::cmp::max(start_at, end_at - chrono::Duration::minutes(10)),
    );
    schedule.push(ScheduledItem {
        at: ending_at,
        action: ScheduledAction::SendEnding { target: end_at },
    });

    sort_schedule(schedule);
}

fn pop_due(
    schedule: &mut Vec<ScheduledItem>,
    now: chrono::DateTime<chrono::Local>,
) -> Vec<ScheduledItem> {
    let split = schedule
        .iter()
        .position(|item| item.at > now)
        .unwrap_or(schedule.len());
    schedule.drain(..split).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Local, TimeZone};

    fn t(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(year, month, day, hour, minute, 0).unwrap()
    }

    #[test]
    fn ending_sorts_before_countdown_at_same_time() {
        let same_at = t(2026, 5, 23, 9, 5);
        let mut schedule = vec![
            ScheduledItem {
                at: same_at,
                action: ScheduledAction::SendCountdown { target: t(2026, 5, 23, 9, 20) },
            },
            ScheduledItem {
                at: same_at,
                action: ScheduledAction::SendEnding { target: t(2026, 5, 23, 9, 15) },
            },
        ];

        sort_schedule(&mut schedule);

        assert!(matches!(schedule[0].action, ScheduledAction::SendEnding { .. }));
        assert!(matches!(schedule[1].action, ScheduledAction::SendCountdown { .. }));
    }

    #[test]
    fn earlier_at_sorts_first() {
        let mut schedule = vec![
            ScheduledItem {
                at: t(2026, 5, 23, 9, 20),
                action: ScheduledAction::SendEnding { target: t(2026, 5, 23, 9, 30) },
            },
            ScheduledItem {
                at: t(2026, 5, 23, 9, 5),
                action: ScheduledAction::SendCountdown { target: t(2026, 5, 23, 9, 20) },
            },
        ];

        sort_schedule(&mut schedule);

        assert_eq!(schedule[0].at, t(2026, 5, 23, 9, 5));
        assert_eq!(schedule[1].at, t(2026, 5, 23, 9, 20));
    }

    #[test]
    fn first_event_schedules_immediate_countdown_and_future_ending() {
        // Event A 9:00-9:15, now is 8:40 (T-20m).
        let now = t(2026, 5, 23, 8, 40);
        let mut schedule = vec![];

        add_calendar_event(&mut schedule, t(2026, 5, 23, 9, 0), t(2026, 5, 23, 9, 15), now);

        assert_eq!(schedule.len(), 2);
        assert_eq!(schedule[0].at, now);
        assert_eq!(
            schedule[0].action,
            ScheduledAction::SendCountdown { target: t(2026, 5, 23, 9, 0) }
        );
        assert_eq!(schedule[1].at, t(2026, 5, 23, 9, 5));
        assert_eq!(
            schedule[1].action,
            ScheduledAction::SendEnding { target: t(2026, 5, 23, 9, 15) }
        );
    }

    #[test]
    fn gap_case_defers_countdown_to_ride_with_prior_ending() {
        // A 9:00-9:15 already on schedule (added at 8:40). Now event B 9:20-9:30 arrives at 9:00.
        let now_b = t(2026, 5, 23, 9, 0);
        let mut schedule = vec![];
        add_calendar_event(
            &mut schedule,
            t(2026, 5, 23, 9, 0),
            t(2026, 5, 23, 9, 15),
            t(2026, 5, 23, 8, 40),
        );

        add_calendar_event(&mut schedule, t(2026, 5, 23, 9, 20), t(2026, 5, 23, 9, 30), now_b);

        // Expected, in (at, ending-before-countdown) order:
        //   8:40  SendCountdown(9:00)
        //   9:05  SendEnding(9:15)
        //   9:05  SendCountdown(9:20)
        //   9:20  SendEnding(9:30)
        assert_eq!(schedule.len(), 4);
        assert_eq!(schedule[0].action, ScheduledAction::SendCountdown { target: t(2026, 5, 23, 9, 0) });
        assert_eq!(schedule[1].at, t(2026, 5, 23, 9, 5));
        assert_eq!(schedule[1].action, ScheduledAction::SendEnding { target: t(2026, 5, 23, 9, 15) });
        assert_eq!(schedule[2].at, t(2026, 5, 23, 9, 5));
        assert_eq!(schedule[2].action, ScheduledAction::SendCountdown { target: t(2026, 5, 23, 9, 20) });
        assert_eq!(schedule[3].at, t(2026, 5, 23, 9, 20));
        assert_eq!(schedule[3].action, ScheduledAction::SendEnding { target: t(2026, 5, 23, 9, 30) });
    }

    #[test]
    fn overlap_case_sends_countdown_immediately() {
        // A 9:00-9:30 already on schedule (added at 8:40). Event B 9:20-9:40 arrives at 9:10,
        // starting while A is still running. B's countdown must fire immediately so it is not
        // hidden behind A's 9:20 "ending soon" indicator.
        let now_b = t(2026, 5, 23, 9, 10);
        let mut schedule = vec![];
        add_calendar_event(
            &mut schedule,
            t(2026, 5, 23, 9, 0),
            t(2026, 5, 23, 9, 30),
            t(2026, 5, 23, 8, 40),
        );

        add_calendar_event(&mut schedule, t(2026, 5, 23, 9, 20), t(2026, 5, 23, 9, 40), now_b);

        // Expected, in (at, ending-before-countdown) order:
        //   8:40  SendCountdown(9:00)   (from A)
        //   9:10  SendCountdown(9:20)   (from B, immediate — not deferred to 9:20)
        //   9:20  SendEnding(9:30)      (from A, kept)
        //   9:30  SendEnding(9:40)      (from B)
        assert_eq!(schedule.len(), 4);
        assert_eq!(schedule[0].at, t(2026, 5, 23, 8, 40));
        assert_eq!(schedule[0].action, ScheduledAction::SendCountdown { target: t(2026, 5, 23, 9, 0) });
        assert_eq!(schedule[1].at, now_b);
        assert_eq!(schedule[1].action, ScheduledAction::SendCountdown { target: t(2026, 5, 23, 9, 20) });
        assert_eq!(schedule[2].at, t(2026, 5, 23, 9, 20));
        assert_eq!(schedule[2].action, ScheduledAction::SendEnding { target: t(2026, 5, 23, 9, 30) });
        assert_eq!(schedule[3].at, t(2026, 5, 23, 9, 30));
        assert_eq!(schedule[3].action, ScheduledAction::SendEnding { target: t(2026, 5, 23, 9, 40) });
    }

    #[test]
    fn chained_case_removes_prior_ending_and_sends_countdown_immediately() {
        // A 10:00-10:15 already on schedule (added at 9:40). Event B 10:15-10:30 arrives at 9:55.
        let now_b = t(2026, 5, 23, 9, 55);
        let mut schedule = vec![];
        add_calendar_event(
            &mut schedule,
            t(2026, 5, 23, 10, 0),
            t(2026, 5, 23, 10, 15),
            t(2026, 5, 23, 9, 40),
        );

        add_calendar_event(&mut schedule, t(2026, 5, 23, 10, 15), t(2026, 5, 23, 10, 30), now_b);

        // A's SendEnding(target=10:15) should have been removed. Expected:
        //   9:40   SendCountdown(10:00)   (from A)
        //   9:55   SendCountdown(10:15)   (from B, immediate)
        //   10:20  SendEnding(10:30)      (from B)
        assert_eq!(schedule.len(), 3);
        assert_eq!(schedule[0].at, t(2026, 5, 23, 9, 40));
        assert_eq!(schedule[0].action, ScheduledAction::SendCountdown { target: t(2026, 5, 23, 10, 0) });
        assert_eq!(schedule[1].at, now_b);
        assert_eq!(schedule[1].action, ScheduledAction::SendCountdown { target: t(2026, 5, 23, 10, 15) });
        assert_eq!(schedule[2].at, t(2026, 5, 23, 10, 20));
        assert_eq!(schedule[2].action, ScheduledAction::SendEnding { target: t(2026, 5, 23, 10, 30) });
    }

    #[test]
    fn past_ending_window_clamps_to_now() {
        // Event arrives less than 10m before its end (degenerate / late delivery).
        let now = t(2026, 5, 23, 9, 10);
        let mut schedule = vec![];

        add_calendar_event(&mut schedule, t(2026, 5, 23, 9, 0), t(2026, 5, 23, 9, 15), now);

        // SendEnding's `at` would be 9:05 but is clamped to now (9:10).
        let ending = schedule
            .iter()
            .find(|item| matches!(item.action, ScheduledAction::SendEnding { .. }))
            .unwrap();
        assert_eq!(ending.at, now);
    }

    #[test]
    fn short_event_delays_ending_to_event_start() {
        // Event shorter than 10m: end_at - 10m precedes start_at, so the ending
        // should fire at start_at, not before the event has even begun.
        let now = t(2026, 5, 23, 8, 40);
        let mut schedule = vec![];

        add_calendar_event(&mut schedule, t(2026, 5, 23, 9, 0), t(2026, 5, 23, 9, 5), now);

        let ending = schedule
            .iter()
            .find(|item| matches!(item.action, ScheduledAction::SendEnding { .. }))
            .unwrap();
        assert_eq!(ending.at, t(2026, 5, 23, 9, 0));
    }

    #[test]
    fn pop_due_returns_items_at_or_before_now_and_removes_them() {
        let now = t(2026, 5, 23, 9, 5);
        let mut schedule = vec![
            ScheduledItem {
                at: t(2026, 5, 23, 9, 0),
                action: ScheduledAction::SendCountdown { target: t(2026, 5, 23, 9, 0) },
            },
            ScheduledItem {
                at: t(2026, 5, 23, 9, 5),
                action: ScheduledAction::SendEnding { target: t(2026, 5, 23, 9, 15) },
            },
            ScheduledItem {
                at: t(2026, 5, 23, 9, 5),
                action: ScheduledAction::SendCountdown { target: t(2026, 5, 23, 9, 20) },
            },
            ScheduledItem {
                at: t(2026, 5, 23, 9, 20),
                action: ScheduledAction::SendEnding { target: t(2026, 5, 23, 9, 30) },
            },
        ];

        let due = pop_due(&mut schedule, now);

        assert_eq!(due.len(), 3);
        assert_eq!(schedule.len(), 1);
        assert_eq!(schedule[0].at, t(2026, 5, 23, 9, 20));
    }

    #[test]
    fn pop_due_returns_empty_when_nothing_due() {
        let now = t(2026, 5, 23, 9, 0);
        let mut schedule = vec![ScheduledItem {
            at: t(2026, 5, 23, 9, 5),
            action: ScheduledAction::SendEnding { target: t(2026, 5, 23, 9, 15) },
        }];

        let due = pop_due(&mut schedule, now);

        assert!(due.is_empty());
        assert_eq!(schedule.len(), 1);
    }
}
