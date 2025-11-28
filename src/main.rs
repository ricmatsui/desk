use kameo::actor::Actor;
use kameo::prelude::*;
use kameo_actors::broker;
use tracing_subscriber::EnvFilter;

mod apps;
mod backlight;
mod circuit_playground;
mod home_assistant;
mod light;
mod macropad;
mod raylib_manager;
mod restarting_manager;
mod serial_sink;
mod thinkink;
mod toggl;
mod unicorn;
mod urban;

#[derive(Debug, Clone)]
pub enum BrokerMessage {
    TimeEntryStarted(TimeEntryStarted),
    TimeEntryStopped,
    TimeEntryTimeUpdated(TimeEntryTimeUpdated),
    CalendarEventUpcoming(CalendarEventUpcoming),
    StartCountdown(i64),
    Message(Message),
    ReadInbox,
    ClearInbox,
    StartClock,
    ServoX(u32),
    ServoY(u32),
}

#[derive(Debug, Clone)]
pub struct Message {
    pub text: String,
    pub effects: Vec<String>,
    pub read: bool,
}

#[derive(Debug, Clone)]
pub struct TimeEntryStarted {
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct TimeEntryTimeUpdated {
    pub minutes: i64,
}

#[derive(Debug, Clone)]
pub struct CalendarEventUpcoming {
    pub description: String,
    pub start_at: chrono::DateTime<chrono::Local>,
    pub end_at: chrono::DateTime<chrono::Local>,
}

pub enum RaylibRequest {
    RenderThinkInkImage,
}

pub enum RaylibResponse {
    ThinkInkImage(Vec<u8>),
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("kameo=trace,reqwest=trace,info".parse::<EnvFilter>().unwrap())
        .without_time()
        .with_target(true)
        .init();

    unsafe {
        raylib::ffi::SetTraceLogLevel(raylib::ffi::TraceLogLevel::LOG_TRACE as i32);
        raylib::ffi::SetTraceLogCallback(Some(raylib_trace_log_custom));
    }

    let (raylib_transmit, raylib_receive) = tokio::sync::mpsc::channel::<RaylibRequest>(64);
    let (raylib_manager_transmit, raylib_manager_receive) =
        tokio::sync::mpsc::channel::<RaylibResponse>(64);

    let tokio_thread = std::thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(tokio_main(raylib_transmit, raylib_manager_receive))
            .unwrap();
    });

    raylib_main(raylib_manager_transmit, raylib_receive);

    tokio_thread.join().unwrap();
}

macro_rules! restarting {
    ($type:ty, ($($var:ident),* $(,)?)) => {
        restarting_manager::RestartingManager::<$type>::spawn((
            Box::new({
                $( let $var = $var.clone(); )*
                move || ( $( $var.clone(), )* )
            }),
        ))
    };
}

#[async_trait::async_trait]
trait WaitableActorRef {
    async fn wait_for_shutdown(&self);
}

#[async_trait::async_trait]
impl<A: Actor> WaitableActorRef for ActorRef<A> {
    async fn wait_for_shutdown(&self) {
        self.wait_for_shutdown().await;
    }
}

async fn tokio_main(
    raylib_transmit: tokio::sync::mpsc::Sender<RaylibRequest>,
    raylib_manager_receive: tokio::sync::mpsc::Receiver<RaylibResponse>,
) -> Result<(), Box<dyn std::error::Error>> {
    ActorSwarm::bootstrap()?
        .listen_on("/ip4/0.0.0.0/udp/8020/quic-v1".parse()?)
        .await?;

    let broker_ref = broker::Broker::spawn(broker::Broker::new(
        kameo_actors::DeliveryStrategy::Guaranteed,
    ));

    let raylib_manager_ref =
        raylib_manager::RaylibManager::spawn((raylib_transmit.clone(), raylib_manager_receive));

    let actor_refs: Vec<Box<dyn WaitableActorRef>> = vec![
        Box::new(broker_ref.clone()),
        Box::new(raylib_manager_ref.clone()),
        Box::new(restarting!(backlight::Backlight, ())),
        Box::new(restarting!(macropad::Macropad, (broker_ref,))),
        Box::new(restarting!(circuit_playground::CircuitPlayground, ())),
        Box::new(restarting!(
            thinkink::ThinkInk,
            (broker_ref, raylib_manager_ref)
        )),
        Box::new(restarting!(unicorn::Unicorn, (broker_ref,))),
        Box::new(restarting!(home_assistant::HomeAssistant, (broker_ref,))),
        Box::new(restarting!(urban::Urban, ())),
    ];

    for actor_ref in actor_refs {
        actor_ref.wait_for_shutdown().await;
    }

    Ok(())
}

fn raylib_main(
    raylib_manager_transmit: tokio::sync::mpsc::Sender<RaylibResponse>,
    mut raylib_receive: tokio::sync::mpsc::Receiver<RaylibRequest>,
) {
    while let Some(request) = raylib_receive.blocking_recv() {
        match request {
            RaylibRequest::RenderThinkInkImage => {
                apps::thinkink_image::thinkink_image(&raylib_manager_transmit);
            }
        }
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
