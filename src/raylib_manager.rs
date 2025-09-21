use kameo::error::Infallible;
use kameo::prelude::*;

pub struct RaylibManager {
    transmit: tokio::sync::mpsc::Sender<crate::RaylibRequest>,
    receive: tokio::sync::mpsc::Receiver<crate::RaylibResponse>,
}

impl Actor for RaylibManager {
    type Args = (
        tokio::sync::mpsc::Sender<crate::RaylibRequest>,
        tokio::sync::mpsc::Receiver<crate::RaylibResponse>,
    );
    type Error = Infallible;

    async fn on_start(state: Self::Args, _actor_ref: ActorRef<Self>) -> Result<Self, Self::Error> {
        let (transmit, receive) = state;

        Ok(Self { transmit, receive })
    }
}

pub struct RenderThinkInkImage;

impl Message<RenderThinkInkImage> for RaylibManager {
    type Reply = Vec<u8>;

    async fn handle(
        &mut self,
        _message: RenderThinkInkImage,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        tracing::info!("-> render thinkink image");
        self.transmit
            .send(crate::RaylibRequest::RenderThinkInkImage)
            .await
            .unwrap();

        let response = self.receive.recv().await.unwrap();

        match response {
            crate::RaylibResponse::ThinkInkImage(data) => {
                tracing::info!("<- thinkink image");
                data
            }
        }
    }
}
