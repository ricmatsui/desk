use chrono::Timelike;
use kameo::error::Infallible;
use kameo::prelude::*;

pub struct Light {
    thinkink_ref: ActorRef<crate::thinkink::ThinkInk>,
    light_spline: splines::Spline<f32, f32>,
}

pub struct Tick;

impl Actor for Light {
    type Args = (ActorRef<crate::thinkink::ThinkInk>,);
    type Error = Infallible;

    async fn on_start(state: Self::Args, actor_ref: ActorRef<Self>) -> Result<Self, Self::Error> {
        let thinkink_ref = state.0;

        let light_spline = splines::Spline::from_vec(vec![
            splines::Key::new(0.0, 0.0, splines::Interpolation::Step(1.0)),
            splines::Key::new(to_seconds(6, 0, 0), 0.0, splines::Interpolation::Cosine),
            splines::Key::new(to_seconds(6, 1, 0), 4096.0, splines::Interpolation::Cosine),
            splines::Key::new(to_seconds(7, 0, 0), 4096.0, splines::Interpolation::Cosine),
            splines::Key::new(to_seconds(7, 1, 0), 8190.0, splines::Interpolation::Cosine),
            splines::Key::new(to_seconds(21, 0, 0), 8190.0, splines::Interpolation::Cosine),
            splines::Key::new(to_seconds(21, 1, 0), 4096.0, splines::Interpolation::Cosine),
            splines::Key::new(
                to_seconds(23, 20, 0),
                4096.0,
                splines::Interpolation::Cosine,
            ),
            splines::Key::new(to_seconds(23, 21, 0), 0.0, splines::Interpolation::Cosine),
            splines::Key::new(to_seconds(24, 0, 0), 0.0, splines::Interpolation::default()),
        ]);

        actor_ref.tell(Tick).try_send().unwrap();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));

            loop {
                interval.tick().await;

                if actor_ref.tell(Tick).await.is_err() {
                    break;
                }
            }
        });

        Ok(Self {
            light_spline,
            thinkink_ref,
        })
    }
}

impl Message<Tick> for Light {
    type Reply = ();

    async fn handle(
        &mut self,
        _message: Tick,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        let now = chrono::Local::now();

        let target_value = self
            .light_spline
            .sample(to_seconds(now.hour(), now.minute(), now.second()) as f32)
            .unwrap() as u32;

        self.thinkink_ref
            .tell(crate::thinkink::UpdateLight {
                target_value,
                speed: 300,
            })
            .await
            .unwrap();
    }
}

fn to_seconds(hour: u32, minute: u32, second: u32) -> f32 {
    hour as f32 * 3600.0 + minute as f32 * 60.0 + second as f32
}
