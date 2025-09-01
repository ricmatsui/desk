#[cfg(feature = "pi")]
use rppal::pwm;
#[cfg(feature = "pi")]
use std::process::Command;

use kameo::error::Infallible;
use kameo::prelude::*;

pub struct Backlight {
    #[cfg(feature = "pi")]
    pwm: pwm::Pwm,

    handle_count: usize,
}

pub struct Request;
pub struct Release;

pub struct SetEnabled(pub bool);

impl Actor for Backlight {
    type Args = ();
    type Error = Infallible;

    async fn on_start(_state: Self::Args, _actor_ref: ActorRef<Self>) -> Result<Self, Self::Error> {
        #[cfg(feature = "pi")]
        {
            let pwm = pwm::Pwm::with_frequency(
                pwm::Channel::Pwm1,
                600_000.0,
                0.0,
                pwm::Polarity::Normal,
                true,
            )
            .unwrap();

            Ok(Self {
                pwm,
                handle_count: 0,
            })
        }

        #[cfg(not(feature = "pi"))]
        {
            Ok(Self { handle_count: 0 })
        }
    }
}

impl Message<Request> for Backlight {
    type Reply = ();

    async fn handle(
        &mut self,
        _message: Request,
        context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        if self.handle_count == 0 {
            context.actor_ref().tell(SetEnabled(true)).await.unwrap();
        }

        self.handle_count += 1;
    }
}

impl Message<Release> for Backlight {
    type Reply = ();

    async fn handle(
        &mut self,
        _message: Release,
        context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.handle_count -= 1;

        if self.handle_count == 0 {
            context.actor_ref().tell(SetEnabled(false)).await.unwrap();
        }
    }
}

impl Message<SetEnabled> for Backlight {
    type Reply = ();

    async fn handle(
        &mut self,
        message: SetEnabled,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        let enabled = message.0;

        tracing::info!("set enabled {}", enabled);

        #[cfg(feature = "pi")]
        {
            Command::new("xset")
                .args(["dpms", "force", if enabled { "on" } else { "off" }])
                .status()
                .unwrap();
            self.pwm
                .set_duty_cycle(if enabled { 0.5 } else { 0.0 })
                .unwrap();
        }
    }
}
