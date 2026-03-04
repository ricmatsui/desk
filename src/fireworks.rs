use chrono::Datelike;
use chrono::TimeZone;
use kameo::error::Infallible;
use kameo::prelude::*;
use kameo_actors::broker;

pub struct Fireworks {
    broker_ref: ActorRef<broker::Broker<crate::BrokerMessage>>,
}

impl Actor for Fireworks {
    type Args = (ActorRef<broker::Broker<crate::BrokerMessage>>,);
    type Error = Infallible;

    async fn on_start(state: Self::Args, actor_ref: ActorRef<Self>) -> Result<Self, Self::Error> {
        let broker_ref = state.0;

        tokio::spawn(async move {
            loop {
                let current_date = chrono::Local::now();

                let next_year = chrono::Local
                    .from_local_datetime(
                        &chrono::NaiveDate::from_ymd_opt(current_date.year() + 1, 1, 1)
                            .unwrap()
                            .and_hms_opt(0, 0, 0)
                            .unwrap(),
                    )
                    .unwrap();

                let sleep_time = (next_year - current_date - chrono::Duration::hours(1))
                    .to_std()
                    .unwrap_or(std::time::Duration::from_secs(0));

                tracing::info!("fireworks next year timer sleeping for {:?}", sleep_time);
                tokio::time::sleep(sleep_time).await;

                if actor_ref
                    .tell(NextYearTimer(next_year.timestamp()))
                    .await
                    .is_err()
                {
                    break;
                }

                let sleep_time = (next_year - current_date)
                    .to_std()
                    .unwrap_or(std::time::Duration::from_secs(0));

                tracing::info!("fireworks next year sleeping for {:?}", sleep_time);
                tokio::time::sleep(sleep_time).await;

                if actor_ref.tell(NextYear).await.is_err() {
                    break;
                }

                tokio::time::sleep(std::time::Duration::from_secs(120)).await;
            }
        });

        Ok(Self { broker_ref })
    }
}

pub struct NextYearTimer(i64);

impl Message<NextYearTimer> for Fireworks {
    type Reply = ();

    async fn handle(
        &mut self,
        message: NextYearTimer,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.broker_ref
            .tell(broker::Publish {
                topic: "countdown".parse().unwrap(),
                message: crate::BrokerMessage::StartTimestampCountdown(message.0),
            })
            .await
            .unwrap();
    }
}

pub struct NextYear;

impl Message<NextYear> for Fireworks {
    type Reply = ();

    async fn handle(
        &mut self,
        _message: NextYear,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        tracing::info!("NextYear started");
        self.broker_ref
            .tell(broker::Publish {
                topic: "fireworks".parse().unwrap(),
                message: crate::BrokerMessage::StartFireworks,
            })
            .await
            .unwrap();

        tokio::time::sleep(std::time::Duration::from_secs(60)).await;

        tracing::info!("NextYear ended");
        self.broker_ref
            .tell(broker::Publish {
                topic: "fireworks".parse().unwrap(),
                message: crate::BrokerMessage::StopFireworks,
            })
            .await
            .unwrap();
    }
}
