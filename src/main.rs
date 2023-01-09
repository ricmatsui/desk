#[cfg(feature = "reloader")]
use {std::sync::mpsc::channel, std::thread};

use std::{env, time};

#[cfg(feature = "reloader")]
use hot_lib::{draw, handle_reload, init, update, ApiClient};
#[cfg(not(feature = "reloader"))]
use lib::{draw, init, update, ApiClient};

#[cfg(feature = "reloader")]
#[hot_lib_reloader::hot_module(dylib = "lib")]
mod hot_lib {
    hot_functions_from_file!("lib/src/lib.rs");

    #[lib_change_subscription]
    pub fn subscribe() -> hot_lib_reloader::LibReloadObserver {}

    pub use lib::State;

    pub use lib::ApiClient;
}

fn main() {
    simple_logger::SimpleLogger::new()
        .with_module_level("rustls", log::LevelFilter::Warn)
        .with_module_level("ureq", log::LevelFilter::Warn)
        .env()
        .init()
        .unwrap();

    let api_client = UreqApiClient {
        request_agent: ureq::AgentBuilder::new()
            .timeout_read(time::Duration::from_secs(5))
            .timeout_write(time::Duration::from_secs(5))
            .build(),
    };

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

    let mut state = init(&mut rl, &thread, Box::new(api_client));

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
}

struct UreqApiClient {
    request_agent: ureq::Agent,
}

impl ApiClient for UreqApiClient {
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
}
