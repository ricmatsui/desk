use kameo::error::Infallible;
use kameo::prelude::*;
use kameo_actors::broker;

use crate::raylib_manager;

/// Lets the message arrive on the unicorn before it's heard
const SOUND_DELAY: std::time::Duration = std::time::Duration::from_secs(1);

/// Plays the sound attached to each inbox message on the hub's speaker
pub struct Speaker {
    raylib_manager_ref: ActorRef<raylib_manager::RaylibManager>,
}

impl Actor for Speaker {
    type Args = (
        ActorRef<broker::Broker<crate::BrokerMessage>>,
        ActorRef<raylib_manager::RaylibManager>,
    );
    type Error = Infallible;

    fn prepare() -> PreparedActor<Self> {
        Self::prepare_with_mailbox(mailbox::unbounded())
    }

    async fn on_start(state: Self::Args, actor_ref: ActorRef<Self>) -> Result<Self, Self::Error> {
        let (broker_ref, raylib_manager_ref) = state;

        broker_ref
            .tell(broker::Subscribe {
                topic: "message".parse().unwrap(),
                recipient: actor_ref.clone().recipient(),
            })
            .await
            .unwrap();

        Ok(Self { raylib_manager_ref })
    }
}

impl Message<crate::BrokerMessage> for Speaker {
    type Reply = ();

    async fn handle(
        &mut self,
        message: crate::BrokerMessage,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        match message {
            crate::BrokerMessage::Message(crate::Message {
                sound: Some(sound), ..
            }) => {
                tracing::info!("play sound {} in {:?}", sound, SOUND_DELAY);

                // Wait outside the handler so later messages aren't held up
                let raylib_manager_ref = self.raylib_manager_ref.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(SOUND_DELAY).await;

                    if raylib_manager_ref
                        .tell(raylib_manager::PlaySound(sound))
                        .await
                        .is_err()
                    {
                        tracing::warn!("raylib manager gone, sound dropped");
                    }
                });
            }
            _ => {}
        }
    }
}
