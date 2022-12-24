#[cfg(feature = "reloader")]
use {std::sync::mpsc::channel, std::thread, std::time};

#[cfg(feature = "reloader")]
use hot_lib::*;
#[cfg(not(feature = "reloader"))]
use lib::*;

#[cfg(feature = "reloader")]
#[hot_lib_reloader::hot_module(dylib = "lib")]
mod hot_lib {
    hot_functions_from_file!("lib/src/lib.rs");

    #[lib_change_subscription]
    pub fn subscribe() -> hot_lib_reloader::LibReloadObserver {}

    pub use lib::State;
}

fn main() {
    #[cfg(feature = "reloader")]
    let mut state = hot_lib::init();
    #[cfg(not(feature = "reloader"))]
    let mut state = lib::init();

    simple_logger::SimpleLogger::new().env().init().unwrap();

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

    while !rl.window_should_close() {
        update(&mut state, &rl);

        let mut d = rl.begin_drawing(&thread);

        draw(&state, &mut d);

        #[cfg(feature = "reloader")]
        {
            if reload_rx.try_recv().is_ok() {
                reload_rx.recv().unwrap();
            }
        }
    }

    #[cfg(feature = "reloader")]
    {
        exit_tx.send(0).unwrap();
        reload_watcher.join().unwrap();
    }
}
