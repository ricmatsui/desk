use btleplug::api::{Central as _, Manager as _, Peripheral as _};
use futures::stream::StreamExt as _;
use packed_struct::prelude::*;
use std::{env, rc::Rc, thread, time};

#[cfg(feature = "reloader")]
use hot_lib::{draw, handle_reload, init, update, ApiClient as LibApiClient};
#[cfg(not(feature = "reloader"))]
use lib::{draw, init, update, ApiClient as LibApiClient};

#[cfg(feature = "reloader")]
use std::sync::mpsc::channel;

fn main() {
    simple_logger::SimpleLogger::new()
        .with_module_level("rustls", log::LevelFilter::Warn)
        .with_module_level("ureq", log::LevelFilter::Warn)
        .with_module_level("serde_xml_rs", log::LevelFilter::Warn)
        .with_module_level("btleplug", log::LevelFilter::Warn)
        .env()
        .init()
        .unwrap();

    #[cfg(feature = "reloader")]
    hot_lib::setup_logger(log::logger(), log::max_level()).unwrap();

    #[cfg(feature = "reloader")]
    let (reload_tx, reload_rx) = channel();
    #[cfg(feature = "reloader")]
    let (exit_tx, exit_rx) = channel();
    #[cfg(feature = "reloader")]
    let reload_watcher = thread::spawn(move || {
        let lib_observer = hot_lib::subscribe();

        loop {
            if let Some(update_blocker) =
                lib_observer.wait_for_about_to_reload_timeout(time::Duration::from_millis(300))
            {
                reload_tx.send(0).unwrap();
                drop(update_blocker);
                lib_observer.wait_for_reload();
                hot_lib::setup_logger(log::logger(), log::max_level()).unwrap();
                reload_tx.send(0).unwrap();
            }

            if exit_rx.try_recv().is_ok() {
                break;
            }
        }
    });

    #[cfg(feature = "pi")]
    let (width, height) = (240, 240);
    #[cfg(not(feature = "pi"))]
    let (width, height) = (600, 320);

    let (mut rl, thread) = raylib::init().size(width, height).title("Desk Pi").build();

    rl.set_target_fps(60);

    #[cfg(feature = "pi")]
    rl.hide_cursor();

    let api_client = Rc::new(ApiClient::new());
    let mut state = init(&mut rl, &thread, api_client.clone());

    while !rl.window_should_close() && rl.get_time() < 82700 as f64 {
        lib::macropad::open_macropad(&mut state.macropad);

        update(&mut state, &mut rl, &thread);

        {
            let mut d = rl.begin_drawing(&thread);

            draw(&mut state, &mut d, &thread);
        }

        #[cfg(feature = "reloader")]
        {
            if reload_rx.try_recv().is_ok() {
                reload_rx.recv().unwrap();
                handle_reload(&mut state, &mut rl, &thread);
            }
        }
    }

    #[cfg(feature = "reloader")]
    {
        exit_tx.send(0).unwrap();
        reload_watcher.join().unwrap();
    }

    drop(state);

    let client = Rc::try_unwrap(api_client).unwrap();
    client.shutdown();
}

#[cfg(feature = "reloader")]
#[hot_lib_reloader::hot_module(dylib = "lib")]
mod hot_lib {
    hot_functions_from_file!("lib/src/lib.rs");

    #[lib_change_subscription]
    pub fn subscribe() -> hot_lib_reloader::LibReloadObserver {}

    pub use lib::State;

    pub use lib::ApiClient;
}

#[derive(Debug)]
struct PuckImageCommand {
    image: lib::puck::PuckImage,
    response_tx: tokio::sync::oneshot::Sender<Result<(), String>>,
}

#[derive(Debug)]
struct ApiClient {
    request_agent: ureq::Agent,
    pub puck_image_tx: Option<tokio::sync::mpsc::Sender<PuckImageCommand>>,
    pub bluetooth_thread: std::thread::JoinHandle<()>,
}

impl ApiClient {
    fn new() -> Self {
        let (bluetooth_thread, puck_image_tx) = start_bluetooth();

        ApiClient {
            request_agent: ureq::AgentBuilder::new()
                .timeout_read(time::Duration::from_secs(5))
                .timeout_write(time::Duration::from_secs(5))
                .build(),
            puck_image_tx: Some(puck_image_tx),
            bluetooth_thread,
        }
    }

    fn shutdown(mut self) {
        self.puck_image_tx = None;
        self.bluetooth_thread.join().unwrap();
    }
}

impl LibApiClient for ApiClient {
    fn make_toggl_request(
        &self,
        method: &str,
        path: &str,
        body: Option<&json::JsonValue>,
    ) -> json::JsonValue {
        let request = self
            .request_agent
            .request(
                method,
                &format!("{}{}", "https://api.track.toggl.com/", path),
            )
            .set("Content-Type", "application/json")
            .set(
                "Authorization",
                &format!("Basic {}", base64::encode(env::var("TOGGL_AUTH").unwrap())),
            );

        let result = match body {
            Some(body) => {
                let body_string = body.dump();
                log::debug!(target: "toggl", "-> {} {}", path, &body_string);
                request.send_string(&body_string)
            }
            None => {
                log::debug!(target: "toggl", "-> {}", path);
                request.call()
            }
        };

        let response_string = result.unwrap().into_string().unwrap();

        log::debug!(target: "toggl", "<- {}", response_string);

        json::parse(&response_string).unwrap()
    }

    fn send_puck_image(&self, image: lib::puck::PuckImage) {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel::<Result<(), String>>();

        log::debug!(target: "puck", "-> image");

        self.puck_image_tx
            .as_ref()
            .unwrap()
            .blocking_send(PuckImageCommand { image, response_tx })
            .unwrap();

        response_rx.blocking_recv().unwrap().unwrap();

        log::debug!(target: "puck", "<- done");
    }
}

fn start_bluetooth() -> (
    std::thread::JoinHandle<()>,
    tokio::sync::mpsc::Sender<PuckImageCommand>,
) {
    let (puck_image_tx, puck_image_rx) = tokio::sync::mpsc::channel::<PuckImageCommand>(1);

    let thread = thread::spawn(|| bluetooth_main(puck_image_rx));

    (thread, puck_image_tx)
}

fn bluetooth_main(mut puck_image_rx: tokio::sync::mpsc::Receiver<PuckImageCommand>) {
    let tokio_runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    tokio_runtime.block_on(async {
        while let Some(command) = puck_image_rx.recv().await {
            let puck = connect_to_puck().await;
            send_image(&puck, &command.image).await;
            puck.disconnect().await.unwrap();

            command.response_tx.send(Ok(())).unwrap();
        }
    });
}

async fn connect_to_puck() -> btleplug::platform::Peripheral {
    let manager = btleplug::platform::Manager::new().await.unwrap();
    let adapters = manager.adapters().await.unwrap();
    let central = adapters.into_iter().nth(0).unwrap();
    let mut events = central.events().await.unwrap();

    log::debug!(target: "bluetooth", "-> start scan");
    central
        .start_scan(btleplug::api::ScanFilter {
            services: vec![uuid::Uuid::parse_str("6e400001-b5a3-f393-e0a9-e50e24dcca9e").unwrap()],
        })
        .await
        .unwrap();

    let mut puck: Option<btleplug::platform::Peripheral> = None;
    while let Some(event) = events.next().await {
        match event {
            btleplug::api::CentralEvent::DeviceDiscovered(id) => {
                let p = central.peripheral(&id).await.unwrap();
                if let Some(name) = p.properties().await.unwrap().unwrap().local_name {
                    if name == "Puck.js e1f5" {
                        puck = Some(p);
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    log::debug!(target: "bluetooth", "= device found");

    let p = puck.unwrap();
    central.stop_scan().await.unwrap();
    log::debug!(target: "bluetooth", "<- scan stopped");
    log::debug!(target: "bluetooth", "-> connect");
    p.connect().await.unwrap();
    p.discover_services().await.unwrap();
    log::debug!(target: "bluetooth", "<- connected");
    return p;
}

#[derive(PackedStruct, Debug)]
#[packed_struct(endian = "lsb")]
pub struct ImageChunk {
    offset: u16,
    length: u8,
    buffer: [u8; 17],
}

async fn send_image(puck: &btleplug::platform::Peripheral, image: &lib::puck::PuckImage) {
    let start_characteristic_uuid: uuid::Uuid =
        uuid::Uuid::parse_str("0000abcc-0000-1000-8000-00805f9b34fb").unwrap();
    let data_characteristic_uuid: uuid::Uuid =
        uuid::Uuid::parse_str("0000abcd-0000-1000-8000-00805f9b34fb").unwrap();
    let refresh_characteristic_uuid: uuid::Uuid =
        uuid::Uuid::parse_str("0000abce-0000-1000-8000-00805f9b34fb").unwrap();

    let characteristics = puck.characteristics();

    let start_characteristic = characteristics
        .iter()
        .find(|c| c.uuid == start_characteristic_uuid)
        .unwrap();
    let data_characteristic = characteristics
        .iter()
        .find(|c| c.uuid == data_characteristic_uuid)
        .unwrap();
    let refresh_characteristic = characteristics
        .iter()
        .find(|c| c.uuid == refresh_characteristic_uuid)
        .unwrap();

    log::debug!(target: "bluetooth", "-> start image");
    puck.write(
        &start_characteristic,
        &[],
        btleplug::api::WriteType::WithResponse,
    )
    .await
    .unwrap();
    log::debug!(target: "bluetooth", "<- done");

    log::debug!(target: "bluetooth", "-> data");
    for (index, chunk_data) in image.data.as_slice().chunks(17).enumerate() {
        let mut buffer = [0; 17];

        buffer[0..chunk_data.len()].copy_from_slice(chunk_data);

        let chunk = ImageChunk {
            offset: index as u16 * 17,
            length: chunk_data.len() as u8,
            buffer,
        };

        let packed_data = chunk.pack().unwrap();

        puck.write(
            &data_characteristic,
            packed_data.as_slice(),
            btleplug::api::WriteType::WithResponse,
        )
        .await
        .unwrap();
    }
    log::debug!(target: "bluetooth", "<- done");

    log::debug!(target: "bluetooth", "-> start refresh");
    puck.write(
        &refresh_characteristic,
        &[],
        btleplug::api::WriteType::WithResponse,
    )
    .await
    .unwrap();
    log::debug!(target: "bluetooth", "<- done");
}
