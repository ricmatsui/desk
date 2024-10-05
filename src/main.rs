use btleplug::api::{Central as _, Manager as _, Peripheral as _};
use core::str::FromStr;
use futures::stream::StreamExt as _;
use std::sync::{mpsc, Arc};
use std::{env, io::Read, thread, time};

#[cfg(feature = "pi")]
use rppal::i2c::I2c;

#[cfg(feature = "reloader")]
use hot_lib::{
    draw, handle_reload, init, shutdown, update, ApiClient as LibApiClient, I2cOperation,
    TogglError,
};

#[cfg(not(feature = "reloader"))]
use lib::{draw, init, shutdown, update, ApiClient as LibApiClient, I2cOperation, TogglError};

fn main() {
    simple_logger::SimpleLogger::new()
        .with_module_level("rustls", log::LevelFilter::Debug)
        .with_module_level("ureq", log::LevelFilter::Trace)
        .with_module_level("serde_xml_rs", log::LevelFilter::Warn)
        .with_module_level("btleplug", log::LevelFilter::Warn)
        .with_module_level("bluez_async", log::LevelFilter::Warn)
        .env()
        .init()
        .unwrap();

    #[cfg(feature = "reloader")]
    hot_lib::setup_logger(log::logger(), log::max_level()).unwrap();

    #[cfg(feature = "reloader")]
    let reload_watcher = ReloadWatcher::new();

    unsafe {
        raylib::ffi::SetTraceLogLevel(raylib::ffi::TraceLogLevel::LOG_TRACE as i32);
        raylib::ffi::SetTraceLogCallback(Some(log_custom));
    }

    #[cfg(feature = "pi")]
    let (width, height) = (240, 240);

    #[cfg(not(feature = "pi"))]
    let (width, height) = (1000, 500);

    let (mut rl, thread) = raylib::init().title("Desk Pi").size(width, height).build();

    rl.set_target_fps(30);

    #[cfg(feature = "pi")]
    rl.hide_cursor();

    let api_client = Arc::new(ApiClient::new());
    let mut state = init(&mut rl, &thread, api_client.clone());

    while !rl.window_should_close() && rl.get_time() < 82700 as f64 {
        state.macropad.open_serial();
        state.thinkink.open_serial();
        state.circuit_playground.open_serial();

        update(&mut state, &mut rl, &thread);

        {
            let mut d = rl.begin_drawing(&thread);

            draw(&mut state, &mut d, &thread);
        }

        #[cfg(feature = "reloader")]
        if reload_watcher.check_pending_reload() {
            handle_reload(&mut state, &mut rl, &thread);
        }
    }

    #[cfg(feature = "reloader")]
    reload_watcher.stop();

    shutdown(state);

    let client = Arc::try_unwrap(api_client).unwrap();
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

    pub use lib::TogglError;

    pub use lib::I2cOperation;
}

#[cfg(feature = "pi")]
type LogText = *const u8;

#[cfg(not(feature = "pi"))]
type LogText = *const i8;

pub extern "C" fn log_custom(msg_type: i32, text: LogText, args: *mut raylib::ffi::__va_list_tag) {
    unsafe {
        let formatted_text = vsprintf::vsprintf(text, args).unwrap();
        match std::mem::transmute(msg_type) {
            raylib::ffi::TraceLogLevel::LOG_FATAL => {
                log::error!(target: "raylib", "FATAL: {}", formatted_text)
            }
            raylib::ffi::TraceLogLevel::LOG_ERROR => {
                log::error!(target: "raylib", "{}", formatted_text)
            }
            raylib::ffi::TraceLogLevel::LOG_WARNING => {
                log::warn!(target: "raylib", "{}", formatted_text)
            }
            raylib::ffi::TraceLogLevel::LOG_INFO => {
                log::info!(target: "raylib", "{}", formatted_text)
            }
            raylib::ffi::TraceLogLevel::LOG_DEBUG => {
                log::debug!(target: "raylib", "{}", formatted_text)
            }
            raylib::ffi::TraceLogLevel::LOG_TRACE => {
                log::trace!(target: "raylib", "{}", formatted_text)
            }
            _ => unreachable!(),
        }
    }
}

#[derive(Debug)]
struct BoseSwitchCommand {
    addresses: [macaddr::MacAddr6; 2],
    response_tx: tokio::sync::oneshot::Sender<Result<(), String>>,
}

#[derive(Debug)]
struct ApiClient {
    request_agent: ureq::Agent,
    i2c_tx: Option<mpsc::SyncSender<Vec<I2cOperation>>>,
    i2c_thread: std::thread::JoinHandle<()>,
    bose_switch_tx: Option<tokio::sync::mpsc::Sender<BoseSwitchCommand>>,
    bose_bluetooth_thread: std::thread::JoinHandle<()>,
}

impl ApiClient {
    fn new() -> Self {
        let (i2c_thread, i2c_tx) = start_i2c();
        let (bose_bluetooth_thread, bose_switch_tx) = start_bose_bluetooth();

        ApiClient {
            request_agent: ureq::AgentBuilder::new()
                .max_idle_connections(0)
                .timeout_read(time::Duration::from_secs(10))
                .timeout_write(time::Duration::from_secs(10))
                .build(),
            i2c_tx: Some(i2c_tx),
            i2c_thread,
            bose_switch_tx: Some(bose_switch_tx),
            bose_bluetooth_thread,
        }
    }

    fn shutdown(mut self) {
        self.i2c_tx = None;
        self.i2c_thread.join().unwrap();
        self.bose_switch_tx = None;
        self.bose_bluetooth_thread.join().unwrap();
    }
}

impl LibApiClient for ApiClient {
    fn make_rammb_request(
        &self,
        name: &str,
        date: chrono::DateTime<chrono::Utc>,
    ) -> Result<raylib::core::texture::Image, Box<dyn std::error::Error>> {
        log::debug!(target: "rammb", "-> request {} {}", name, date);
        let times_url = format!("https://rammb-slider.cira.colostate.edu/data/json/{}/full_disk/geocolor/latest_times.json", name);
        let result = self.request_agent.request("GET", &times_url).call();

        let target_timestamp_int = format!("{}", date.format("%Y%m%d%H%M%S"))
            .parse::<i64>()
            .unwrap();

        let times = json::parse(&result.unwrap().into_string().unwrap()).unwrap();

        let timestamp = times["timestamps_int"]
            .members()
            .min_by_key(|&t| (t.as_i64().unwrap() - target_timestamp_int).abs())
            .unwrap()
            .to_string();

        //let timestamp = times["timestamps_int"][0].as_i64().unwrap().to_string();

        //let time = chrono::DateTime::parse_from_str(&format!("{timestamp} +0000"), "%Y%m%d%H%M%S %z").unwrap();

        log::debug!(target: "rammb", "-> timestamp {}", timestamp);
        let url = format!("https://rammb-slider.cira.colostate.edu/data/imagery/{}/{}/{}/{}---full_disk/geocolor/{}/00/000_000.png", &timestamp[0..4], &timestamp[4..6], &timestamp[6..8], &name, &timestamp);

        let request = self
            .request_agent
            .request("GET", &url)
            .set("Content-Type", "image/png");

        let response = request.call().unwrap();

        let length: usize = response.header("Content-Length").unwrap().parse().unwrap();
        println!("length: {}", length);

        let mut bytes: Vec<u8> = Vec::with_capacity(length);

        response
            .into_reader()
            .take(10_000_000)
            .read_to_end(&mut bytes)
            .unwrap();

        assert_eq!(bytes.len(), length);

        let image =
            raylib::core::texture::Image::load_image_from_mem(".png", &bytes, length as i32)?;
        Ok(image)
    }

    fn make_noaa_tile_request(&self, level: u8, x: u8, y: u8) -> raylib::core::texture::Image {
        let request = self
            .request_agent
            .request(
                "GET",
                &format!("https://gis.nnvl.noaa.gov/arcgis/rest/services/TRUE/TRUE_current/ImageServer/tile/{}/{}/{}", level, y, x)
                )
            .set("Content-Type", "image/jpeg");

        let response = request.call().unwrap();

        let length: usize = response.header("Content-Length").unwrap().parse().unwrap();
        println!("length: {}", length);

        let mut bytes: Vec<u8> = Vec::with_capacity(length);

        response
            .into_reader()
            .take(10_000_000)
            .read_to_end(&mut bytes)
            .unwrap();

        assert_eq!(bytes.len(), length);

        raylib::core::texture::Image::load_image_from_mem(".jpeg", &bytes, length as i32).unwrap()
    }

    fn make_noaa_archive_request(
        &self,
        width: u32,
        height: u32,
        date: chrono::DateTime<chrono::Utc>,
    ) -> Result<raylib::core::texture::Image, Box<dyn std::error::Error>> {
        log::debug!(target: "noaa", "-> export image {}", date);
        let request = self
            .request_agent
            .request(
                "GET",
                "https://gis.nnvl.noaa.gov/arcgis/rest/services/TRUE/TRUE_daily_750m/ImageServer/exportImage"
                )
            .query("bbox", "-180.0,-90,180.0,90.0")
            .query("size", &format!("{}x{}", width, height))
            .query("imageSR", "43001")
            .query("time", &format!("{}", date.timestamp_millis()))
            .query("format", "png")
            .query("pixelType", "U8")
            .query("adjustAspectRatio", "true")
            .query("f", "image")
            .set("Content-Type", "image/png");

        let response = request.call()?;

        let length: usize = response
            .header("Content-Length")
            .ok_or("Missing content length header")?
            .parse()?;

        let mut bytes: Vec<u8> = Vec::with_capacity(length);

        response.into_reader().read_to_end(&mut bytes)?;

        assert_eq!(bytes.len(), length);

        let image =
            raylib::core::texture::Image::load_image_from_mem(".png", &bytes, length as i32)?;
        log::debug!(target: "noaa", "<- image {} {:?}", length, image);
        Ok(image)
    }

    fn make_open_meteo_request(&self) -> Result<json::JsonValue, Box<dyn std::error::Error>> {
        let request = self
            .request_agent
            .request("GET", "https://api.open-meteo.com/v1/forecast")
            .query("latitude", &env::var("LATITUDE").unwrap())
            .query("longitude", &env::var("LONGITUDE").unwrap())
            .query("hourly", "temperature_2m,precipitation_probability")
            .query("temperature_unit", "fahrenheit")
            .query("wind_speed_unit", "mph")
            .query("precipitation_unit", "inch")
            .query("timezone", "America/Los_Angeles")
            .query("forecast_days", "2")
            .set("Content-Type", "application/json");

        log::debug!(target: "open_meteo", "-> {}", request.url());
        let result = request.call()?;

        let response_string = result.into_string()?;
        log::debug!(target: "open_meteo", "<- {}", response_string);

        Ok(json::parse(&response_string)?)
    }

    fn make_toggl_request(
        &self,
        method: &str,
        path: &str,
        body: Option<&json::JsonValue>,
    ) -> Result<json::JsonValue, TogglError> {
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

        let response_string = result
            .or_else(|error| {
                log::error!(target: "toggl", "!! call {:?}", error);
                Err(TogglError)
            })?
            .into_string()
            .or_else(|error| {
                log::error!(target: "toggl", "!! string {:?}", error);
                Err(TogglError)
            })?;

        log::debug!(target: "toggl", "<- {}", response_string);

        json::parse(&response_string).or_else(|error| {
            log::error!(target: "toggl", "!! parse {:?}", error);
            Err(TogglError)
        })
    }

    fn switch_bose_devices(&self, addresses: [macaddr::MacAddr6; 2]) {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel::<Result<(), String>>();

        log::debug!(target: "bose", "-> switch");

        self.bose_switch_tx
            .as_ref()
            .unwrap()
            .blocking_send(BoseSwitchCommand {
                addresses,
                response_tx,
            })
            .unwrap();

        response_rx.blocking_recv().unwrap().unwrap();

        log::debug!(target: "bose", "<- done");
    }

    fn enqueue_i2c(&self, operations: Vec<I2cOperation>) {
        self.i2c_tx.as_ref().unwrap().send(operations).unwrap();
    }

    fn send_wake_on_lan(&self) {
        let address = macaddr::MacAddr6::from_str(&env::var("WAKE_ON_LAN_MAC").unwrap())
            .unwrap()
            .into_array();

        log::debug!(target: "wake_on_lan", "-> {:02x?}", address);
        wake_on_lan::MagicPacket::new(&address).send().unwrap();
    }

    fn submit_metrics(&self, metrics: json::JsonValue) {
        let metrics_string = metrics.dump();

        log::debug!(target: "datadog", "-> {}", &metrics_string);

        let result = self
            .request_agent
            .request("POST", "https://api.datadoghq.com/api/v2/series")
            .set("Content-Type", "application/json")
            .set("DD-API-KEY", &env::var("DATADOG_API_KEY").unwrap())
            .send_string(&metrics_string)
            .ok()
            .and_then(|response| response.into_string().ok());

        log::debug!(target: "datadog", "<- {}", &result.unwrap_or(String::from("[error]")));
    }
}

fn start_i2c() -> (
    std::thread::JoinHandle<()>,
    mpsc::SyncSender<Vec<I2cOperation>>,
) {
    let (i2c_tx, i2c_rx) = mpsc::sync_channel::<Vec<I2cOperation>>(100);

    let thread = thread::spawn(move || {
        #[cfg(not(feature = "pi"))]
        while let Ok(_operations) = i2c_rx.recv() {}

        #[cfg(feature = "pi")]
        {
            let mut i2c = I2c::new().unwrap();

            while let Ok(operations) = i2c_rx.recv() {
                for operation in operations {
                    match operation {
                        I2cOperation::SetAddress(address) => {
                            i2c.set_slave_address(address).unwrap();
                        }
                        I2cOperation::WriteByte(command, value) => {
                            i2c.smbus_write_byte(command, value).unwrap();
                        }
                        I2cOperation::Write(buffer) => {
                            i2c.write(&buffer).unwrap();
                        }
                    }
                }
            }
        }
    });

    (thread, i2c_tx)
}

fn start_bose_bluetooth() -> (
    std::thread::JoinHandle<()>,
    tokio::sync::mpsc::Sender<BoseSwitchCommand>,
) {
    let (bose_switch_tx, bose_switch_rx) = tokio::sync::mpsc::channel::<BoseSwitchCommand>(1);

    let thread = thread::spawn(|| bose_bluetooth_main(bose_switch_rx));

    (thread, bose_switch_tx)
}

fn bose_bluetooth_main(mut bose_switch_rx: tokio::sync::mpsc::Receiver<BoseSwitchCommand>) {
    let tokio_runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    tokio_runtime.block_on(async {
        while let Some(command) = bose_switch_rx.recv().await {
            let bose = connect_to_bose().await;
            switch_devices(&bose, &command.addresses).await;
            bose.disconnect().await.unwrap();

            command.response_tx.send(Ok(())).unwrap();
        }
    });
}

async fn connect_to_bose() -> btleplug::platform::Peripheral {
    let manager = btleplug::platform::Manager::new().await.unwrap();
    let adapters = manager.adapters().await.unwrap();
    let central = adapters.into_iter().nth(0).unwrap();
    let mut events = central.events().await.unwrap();

    log::debug!(target: "bose", "-> start scan");
    central
        .start_scan(btleplug::api::ScanFilter {
            services: vec![uuid::Uuid::parse_str("0000fdd2-0000-1000-8000-00805f9b34fb").unwrap()],
        })
        .await
        .unwrap();

    let mut bose: Option<btleplug::platform::Peripheral> = None;
    while let Some(event) = events.next().await {
        match event {
            btleplug::api::CentralEvent::DeviceDiscovered(id) => {
                let p = central.peripheral(&id).await.unwrap();
                if p.address().to_string() == env::var("BOSE_MAC").unwrap() {
                    bose = Some(p);
                    break;
                }
            }
            _ => {}
        }
    }
    drop(events);
    log::debug!(target: "bose", "= device found");

    let p = bose.unwrap();
    central.stop_scan().await.unwrap();
    log::debug!(target: "bose", "<- scan stopped");
    log::debug!(target: "bose", "-> connect");
    p.connect().await.unwrap();
    p.discover_services().await.unwrap();
    log::debug!(target: "bose", "<- connected");
    return p;
}

async fn switch_devices(bose: &btleplug::platform::Peripheral, addresses: &[macaddr::MacAddr6; 2]) {
    let command_characteristic_uuid: uuid::Uuid =
        uuid::Uuid::parse_str("d417c028-9818-4354-99d1-2ac09d074591").unwrap();

    let response_characteristic_uuid: uuid::Uuid =
        uuid::Uuid::parse_str("c65b8f2f-aee2-4c89-b758-bc4892d6f2d8").unwrap();

    let characteristics = bose.characteristics();

    let command_characteristic = characteristics
        .iter()
        .find(|c| c.uuid == command_characteristic_uuid)
        .unwrap();

    let response_characteristic = characteristics
        .iter()
        .find(|c| c.uuid == response_characteristic_uuid)
        .unwrap();

    log::debug!(target: "bose", "-> start switch");

    bose.subscribe(response_characteristic).await.unwrap();
    let mut notifications = bose.notifications().await.unwrap();

    log::debug!(target: "bose", "-> request devices");
    bose.write(
        &command_characteristic,
        &[0x00, 0x04, 0x04, 0x01, 0x00],
        btleplug::api::WriteType::WithResponse,
    )
    .await
    .unwrap();
    log::debug!(target: "bose", "<- write request");

    let mut value: Option<Vec<u8>> = None;

    log::debug!(target: "bose", "= wait notification");
    while let Some(notification) = notifications.next().await {
        log::debug!(target: "bose", "<- notification {:02x?}", notification);

        if notification.uuid == response_characteristic_uuid {
            value = Some(notification.value);
            break;
        }
    }

    log::debug!(target: "bose", "<- received devices");

    let response = value.unwrap();
    log::debug!(target: "bose", "<- response {:02x?}", response);
    let mut data = response.iter().skip(4);
    let data_length = data.next().unwrap();
    data.next().unwrap();

    let paired_count = (data_length - 1) / 6;
    let mut paired_addresses: Vec<macaddr::MacAddr6> = vec![];

    log::debug!(target: "bose", "<- paired count {:?}", paired_count);

    for _ in 0..paired_count {
        paired_addresses.push(macaddr::MacAddr6::from([
            *data.next().unwrap(),
            *data.next().unwrap(),
            *data.next().unwrap(),
            *data.next().unwrap(),
            *data.next().unwrap(),
            *data.next().unwrap(),
        ]));
    }

    log::debug!(target: "bose", "<- paired addresses {:02x?}", paired_addresses);

    let mut connected_addresses: Vec<macaddr::MacAddr6> = vec![];

    for address in paired_addresses {
        bose.write(
            &command_characteristic,
            &[&[0x00, 0x04, 0x05, 0x01, 0x06], address.as_bytes()].concat(),
            btleplug::api::WriteType::WithoutResponse,
        )
        .await
        .unwrap();

        let mut value: Option<Vec<u8>> = None;

        while let Some(notification) = notifications.next().await {
            log::debug!(target: "bose", "<- notification {:02x?}", notification);

            if notification.uuid == response_characteristic_uuid {
                value = Some(notification.value);
                break;
            }
        }

        let response = value.unwrap();
        log::debug!(target: "bose", "<- response {:02x?}", response);
        let mut data = response.iter().skip(11);

        if *data.next().unwrap() == 0x01 {
            connected_addresses.push(address);
        }
    }

    drop(notifications);
    bose.unsubscribe(response_characteristic).await.unwrap();

    for address in connected_addresses {
        log::debug!(target: "bose", "-> disconnect {:02x?}", address);
        bose.write(
            &command_characteristic,
            &[&[0x00, 0x04, 0x02, 0x05, 0x06], address.as_bytes()].concat(),
            btleplug::api::WriteType::WithoutResponse,
        )
        .await
        .unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    }

    for address in addresses {
        log::debug!(target: "bose", "-> connect {:02x?}", address);
        bose.write(
            &command_characteristic,
            &[&[0x00, 0x04, 0x01, 0x05, 0x07, 0x00], address.as_bytes()].concat(),
            btleplug::api::WriteType::WithoutResponse,
        )
        .await
        .unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    }

    log::debug!(target: "bose", "<- done");
}

#[cfg(feature = "reloader")]
struct ReloadWatcher {
    reload_rx: mpsc::Receiver<()>,
    exit_tx: mpsc::Sender<()>,
    watcher_thread: thread::JoinHandle<()>,
}

#[cfg(feature = "reloader")]
impl ReloadWatcher {
    pub fn new() -> Self {
        let (reload_tx, reload_rx) = mpsc::channel::<()>();
        let (exit_tx, exit_rx) = mpsc::channel::<()>();

        let watcher_thread = thread::spawn(move || {
            let lib_observer = hot_lib::subscribe();

            loop {
                if let Some(update_blocker) =
                    lib_observer.wait_for_about_to_reload_timeout(time::Duration::from_millis(300))
                {
                    reload_tx.send(()).unwrap();
                    drop(update_blocker);
                    lib_observer.wait_for_reload();
                    hot_lib::setup_logger(log::logger(), log::max_level()).unwrap();
                    reload_tx.send(()).unwrap();
                }

                if exit_rx.try_recv().is_ok() {
                    break;
                }
            }
        });

        Self {
            reload_rx,
            exit_tx,
            watcher_thread,
        }
    }

    pub fn check_pending_reload(&self) -> bool {
        if self.reload_rx.try_recv().is_ok() {
            self.reload_rx.recv().unwrap();
            return true;
        }

        false
    }

    pub fn stop(self) {
        self.exit_tx.send(()).unwrap();
        self.watcher_thread.join().unwrap();
    }
}
