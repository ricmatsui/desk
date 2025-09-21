use kameo::actor::Actor;
use kameo::error::Infallible;
use kameo::prelude::*;

pub struct RestartingManager<A: Actor> {
    get_child_args: Box<dyn Fn() -> A::Args + Send + Sync>,
    child_ref: ActorRef<A>,
}

struct Tick;

impl<A> Actor for RestartingManager<A>
where
    A: Actor,
{
    type Args = (Box<dyn Fn() -> A::Args + Send + Sync>,);
    type Error = Infallible;

    async fn on_start(state: Self::Args, actor_ref: ActorRef<Self>) -> Result<Self, Self::Error> {
        let get_child_args = state.0;

        let child_ref = RestartingManager::spawn_link(&get_child_args, &actor_ref).await;

        actor_ref.tell(Tick).try_send().unwrap();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));

            loop {
                interval.tick().await;

                if actor_ref.tell(Tick).try_send().is_err() {
                    break;
                }
            }
        });

        Ok(Self {
            get_child_args,
            child_ref,
        })
    }

    async fn on_link_died(
        &mut self,
        actor_ref: WeakActorRef<Self>,
        id: ActorID,
        _reason: ActorStopReason,
    ) -> Result<::core::ops::ControlFlow<kameo::error::ActorStopReason>, Self::Error> {
        tracing::warn!("link died - {:?}", id);
        self.child_ref =
            RestartingManager::spawn_link(&self.get_child_args, &actor_ref.upgrade().unwrap()).await;
        tracing::info!("spawned - {:?}", id);
        Ok(::core::ops::ControlFlow::Continue(()))
    }
}

impl<A> Message<Tick> for RestartingManager<A>
where
    A: Actor,
{
    type Reply = ();

    async fn handle(
        &mut self,
        _message: Tick,
        context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        if self.child_ref.is_alive() {
            return;
        }

        tracing::warn!("link found dead - {:?}", self.child_ref.id());
        self.child_ref =
            RestartingManager::spawn_link(&self.get_child_args, &context.actor_ref()).await;
        tracing::info!("spawned - {:?}", self.child_ref.id());
    }
}

impl<A> RestartingManager<A>
where
    A: Actor,
{
    async fn spawn_link(
        get_child_args: &Box<dyn Fn() -> A::Args + Send + Sync>,
        actor_ref: &ActorRef<Self>,
    ) -> ActorRef<A> {
        let prepared_child = A::prepare();
        let child_ref = prepared_child.actor_ref().clone();
        child_ref.link(&actor_ref).await;
        prepared_child.spawn(get_child_args()).await.unwrap().unwrap();

        child_ref
    }
}
