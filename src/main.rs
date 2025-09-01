use raylib::prelude::*;

use kameo::actor::Actor;
use kameo::error::Infallible;
use kameo::prelude::*;
use kameo_actors::broker;
use tracing_subscriber::EnvFilter;

mod backlight;
mod light;
mod macropad;
mod raylib_actor;
mod thinkink;
mod toggl;

#[derive(Debug, Clone)]
pub enum BrokerMessage {
    TimeEntryStarted(TimeEntryStarted),
    TimeEntryStopped,
    TimeEntryTimeUpdated(TimeEntryTimeUpdated),
}

#[derive(Debug, Clone)]
pub struct TimeEntryStarted {
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct TimeEntryTimeUpdated {
    pub minutes: i64,
}

pub enum RaylibRequest {
    RenderThinkInkImage,
}

pub enum RaylibResponse {
    ThinkInkImage(Vec<u8>),
}

type SpawnFn = Box<
    dyn Fn(ActorRef<RestartingManager>) -> std::pin::Pin<Box<dyn Future<Output = ()> + Send>>
        + Send,
>;

pub struct RestartingManager {
    spawn: SpawnFn,
}

impl Actor for RestartingManager {
    type Args = (SpawnFn,);
    type Error = Infallible;

    async fn on_start(state: Self::Args, actor_ref: ActorRef<Self>) -> Result<Self, Self::Error> {
        let spawn = state.0;

        spawn(actor_ref.clone()).await;

        Ok(Self { spawn })
    }

    async fn on_link_died(
        &mut self,
        actor_ref: WeakActorRef<Self>,
        id: ActorID,
        _reason: ActorStopReason,
    ) -> Result<::core::ops::ControlFlow<kameo::error::ActorStopReason>, Self::Error> {
        println!("link died - {:?}", id);
        (self.spawn)(actor_ref.upgrade().unwrap()).await;
        println!("spawned - {:?}", id);
        Ok(::core::ops::ControlFlow::Continue(()))
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter("kameo=trace,info".parse::<EnvFilter>()?)
        .without_time()
        .with_target(true)
        .init();

    unsafe {
        raylib::ffi::SetTraceLogLevel(raylib::ffi::TraceLogLevel::LOG_TRACE as i32);
        raylib::ffi::SetTraceLogCallback(Some(raylib_trace_log_custom));
    }

    let (raylib_transmit, raylib_receive) = tokio::sync::mpsc::channel::<RaylibRequest>(64);
    let (raylib_actor_transmit, raylib_actor_receive) =
        tokio::sync::mpsc::channel::<RaylibResponse>(64);

    let tokio_thread = std::thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(tokio_main(raylib_transmit, raylib_actor_receive))
            .unwrap();
    });

    raylib_main(raylib_actor_transmit, raylib_receive);

    tokio_thread.join().unwrap();

    Ok(())
}

async fn tokio_main(
    raylib_transmit: tokio::sync::mpsc::Sender<RaylibRequest>,
    raylib_actor_receive: tokio::sync::mpsc::Receiver<RaylibResponse>,
) -> Result<(), Box<dyn std::error::Error>> {
    ActorSwarm::bootstrap()?
        .listen_on("/ip4/0.0.0.0/udp/8020/quic-v1".parse()?)
        .await?;

    let broker_ref = broker::Broker::spawn(broker::Broker::new(
        kameo_actors::DeliveryStrategy::Guaranteed,
    ));

    let raylib_actor_ref =
        raylib_actor::Raylib::spawn((raylib_transmit.clone(), raylib_actor_receive));

    let backlight_ref = backlight::Backlight::spawn(());

    let toggl_ref = toggl::Toggl::spawn((broker_ref.clone(),));

    let macropad_ref = macropad::Macropad::spawn((toggl_ref.clone(),));

    let thinkink_broker_ref = broker_ref.clone();
    let thinkink_raylib_actor_ref = raylib_actor_ref.clone();

    let restarting_thinkink = RestartingManager::spawn((Box::new(move |actor_ref| {
        let thinkink_broker_ref = thinkink_broker_ref.clone();
        let thinkink_raylib_actor_ref = thinkink_raylib_actor_ref.clone();

        Box::pin(async move {
            thinkink::ThinkInk::spawn_link(
                &actor_ref,
                (
                    thinkink_broker_ref.clone(),
                    thinkink_raylib_actor_ref.clone(),
                ),
            )
            .await;
        })
    }),));

    broker_ref.wait_for_shutdown().await;
    raylib_actor_ref.wait_for_shutdown().await;
    backlight_ref.wait_for_shutdown().await;
    toggl_ref.wait_for_shutdown().await;
    macropad_ref.wait_for_shutdown().await;
    restarting_thinkink.wait_for_shutdown().await;

    Ok(())
}

fn raylib_main(
    raylib_actor_transmit: tokio::sync::mpsc::Sender<RaylibResponse>,
    mut raylib_receive: tokio::sync::mpsc::Receiver<RaylibRequest>,
) {
    while let Some(_) = raylib_receive.blocking_recv() {
        let (mut rl, thread) = raylib::init().size(240, 240).title("Desk").build();

        rl.set_target_fps(30);

        let mut d = rl.begin_drawing(&thread);

        d.clear_background(Color::WHITE);
        d.draw_text("Hello, world!", 12, 12, 20, Color::BLACK);

        let image =
            raylib::core::texture::Image::gen_image_color(296 as i32, 128 as i32, Color::WHITE);

        let mut data = vec![0; image.width() as usize * image.height() as usize / 4];

        let image_data = unsafe {
            std::slice::from_raw_parts(
                image.data as *const u8,
                image.width() as usize * image.height() as usize * 2,
            )
        };

        for i in 0..data.len() {
            let mut byte = 0;

            for j in 0..4 {
                let pixel = (image_data[i * 8 + j * 2] & 0b1100) >> 2;

                byte |= pixel << (3 - j) * 2;
            }

            data[i] = byte;
        }

        raylib_actor_transmit
            .blocking_send(RaylibResponse::ThinkInkImage(data))
            .unwrap();
    }
}

#[cfg(feature = "pi")]
type LogText = *const u8;

#[cfg(not(feature = "pi"))]
type LogText = *const i8;

pub extern "C" fn raylib_trace_log_custom(
    msg_type: i32,
    text: LogText,
    args: *mut raylib::ffi::__va_list_tag,
) {
    unsafe {
        let formatted_text = vsprintf::vsprintf(text, args).unwrap();
        match std::mem::transmute(msg_type) {
            raylib::ffi::TraceLogLevel::LOG_FATAL => {
                tracing::error!(target: "raylib", "FATAL: {}", formatted_text)
            }
            raylib::ffi::TraceLogLevel::LOG_ERROR => {
                tracing::error!(target: "raylib", "{}", formatted_text)
            }
            raylib::ffi::TraceLogLevel::LOG_WARNING => {
                tracing::warn!(target: "raylib", "{}", formatted_text)
            }
            raylib::ffi::TraceLogLevel::LOG_INFO => {
                tracing::info!(target: "raylib", "{}", formatted_text)
            }
            raylib::ffi::TraceLogLevel::LOG_DEBUG => {
                tracing::debug!(target: "raylib", "{}", formatted_text)
            }
            raylib::ffi::TraceLogLevel::LOG_TRACE => {
                tracing::trace!(target: "raylib", "{}", formatted_text)
            }
            _ => unreachable!(),
        }
    }
}
