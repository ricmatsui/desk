use raylib::prelude::*;
use std::time::Duration;

use kameo::actor::{Actor, ActorRef};
use kameo::error::Infallible;
use kameo::prelude::*;
use serde::{Deserialize, Serialize};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[derive(Actor, RemoteActor)]
pub struct MyActor {
    count: i64,
}

#[derive(Serialize, Deserialize)]
pub struct Inc {
    amount: u32,
}

#[remote_message("3b9128f1-0593-44a0-b83a-f4188baa05bf")]
impl Message<Inc> for MyActor {
    type Reply = i64;

    async fn handle(&mut self, msg: Inc, _ctx: &mut Context<Self, Self::Reply>) -> Self::Reply {
        println!("incrementing");
        self.count += msg.amount as i64;
        self.count
    }
}

#[derive(Serialize, Deserialize)]
pub struct Dec {
    amount: u32,
}

#[remote_message("20185b42-8645-47d2-8d65-2d1c68d26823")]
impl Message<Dec> for MyActor {
    type Reply = i64;

    async fn handle(&mut self, msg: Dec, _ctx: &mut Context<Self, Self::Reply>) -> Self::Reply {
        println!("decrementing");
        self.count -= msg.amount as i64;
        self.count
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let is_host = match std::env::args().nth(1).as_deref() {
        Some("guest") => false,
        Some("host") => true,
        Some(_) | None => {
            error!("expected either 'host' or 'guest' argument");
            return Ok(());
        }
    };

    let tokio_thread = std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().unwrap();

        let result: Result<(), Box<dyn std::error::Error>> = runtime.block_on(async {
            tracing_subscriber::fmt()
                .with_env_filter("info".parse::<EnvFilter>()?)
                .without_time()
                .with_target(false)
                .init();

            if is_host {
                ActorSwarm::bootstrap()?
                    .listen_on("/ip4/0.0.0.0/udp/8020/quic-v1".parse()?)
                    .await?;
            } else {
                let dial_opts = kameo::remote::dial_opts::DialOpts::unknown_peer_id()
                    .address("/ip4/192.0.2.0/udp/8020/quic-v1".parse()?)
                    .build();

                ActorSwarm::bootstrap()?.dial(dial_opts).await?;
            }

            if is_host {
                let actor_ref = MyActor::spawn(MyActor { count: 0 });
                info!("registering actor");
                actor_ref.register("my_actor").await?;
            } else {
                // Wait for registry to sync
                tokio::time::sleep(Duration::from_millis(50)).await;
            }

            loop {
                if !is_host {
                    let remote_actor_ref = RemoteActorRef::<MyActor>::lookup("my_actor").await?;
                    match remote_actor_ref {
                        Some(remote_actor_ref) => {
                            let count = remote_actor_ref.ask(&Inc { amount: 10 }).await?;
                            println!("Incremented! Count is {count}");
                        }
                        None => {
                            println!("actor not found");
                        }
                    }
                }

                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        });

        result.unwrap();
    });

    for i in 1..10 {
        let (mut rl, thread) = raylib::init().size(640, 480).title("Hello, World").build();

        let mut count = 0;

        rl.set_target_fps(60);

        while !rl.window_should_close() {
            let mut d = rl.begin_drawing(&thread);

            d.clear_background(Color::WHITE);
            d.draw_text("Hello, world!", 12, 12, 20, Color::BLACK);

            count += 1;

            if count > 500 {
                break;
            }
        }

        println!("restarting");
    }

    tokio_thread.join().unwrap();

    Ok(())
}
