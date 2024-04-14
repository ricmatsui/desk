use backlight::Backlight;
use earth::Earth;
use input::Input;
use macropad::MacroPad;
use thinkink::ThinkInk;
use matrix::Matrix;
use pixels::Pixels;
use raylib::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

mod backlight;
mod earth;
mod input;
pub mod macropad;
pub mod thinkink;
mod matrix;
mod pixels;

pub trait ApiClient: Send + Sync {
    fn make_noaa_tile_request(&self, level: u8, x: u8, y: u8) -> Image;
    fn make_rammb_request(
        &self,
        name: &str,
        date: chrono::DateTime<chrono::Utc>,
    ) -> Result<Image, Box<dyn std::error::Error>>;
    fn make_noaa_archive_request(
        &self,
        width: u32,
        height: u32,
        date: chrono::DateTime<chrono::Utc>,
    ) -> Result<Image, Box<dyn std::error::Error>>;
    fn make_toggl_request(
        &self,
        method: &str,
        path: &str,
        body: Option<&json::JsonValue>,
    ) -> Result<json::JsonValue, TogglError>;
    fn switch_bose_devices(&self, addresses: [macaddr::MacAddr6; 2]);
    fn enqueue_i2c(&self, operations: Vec<I2cOperation>);
    fn send_wake_on_lan(&self);
}

#[derive(Debug, Clone)]
pub struct TogglError;

#[derive(Debug, Clone)]
pub enum I2cOperation {
    SetAddress(u16),
    WriteByte(u8, u8),
    Write(Vec<u8>),
}

pub struct State {
    context: Context,
    pixels: pixels::Pixels,
    pub macropad: macropad::MacroPad,
    pub thinkink: thinkink::ThinkInk,
    backlight: backlight::Backlight,
    earth: earth::Earth,
    //matrix: matrix::Matrix,
}

pub struct Context {
    time: f64,
    input: std::rc::Rc<std::cell::RefCell<input::Input>>,
    screen_enabled: bool,
}

#[no_mangle]
pub fn setup_logger(
    logger: &'static dyn log::Log,
    level: log::LevelFilter,
) -> Result<(), log::SetLoggerError> {
    log::set_max_level(level);
    log::set_logger(logger)
}

#[no_mangle]
pub fn init(
    rl: &mut raylib::RaylibHandle,
    thread: &raylib::RaylibThread,
    api_client: std::sync::Arc<dyn ApiClient>,
) -> State {
    let input = Rc::new(RefCell::new(Input::new()));

    State {
        context: Context {
            time: 0.0,
            input: input.clone(),
            screen_enabled: false,
        },
        pixels: Pixels::new(api_client.clone()),
        macropad: MacroPad::new(api_client.clone(), input.clone()),
        thinkink: ThinkInk::new(rl, thread),
        backlight: Backlight::new(),
        earth: Earth::new(rl, thread, api_client.clone()),
        //matrix: Matrix::new(rl, thread, api_client.clone()),
    }
}

#[no_mangle]
pub fn update(state: &mut State, rl: &mut raylib::RaylibHandle, thread: &raylib::RaylibThread) {
    state.context.time = rl.get_time();
    state.context.input.borrow_mut().update(rl);

    let input = state.context.input.borrow();

    if !state.context.screen_enabled
        && input.is_active()
        && !input.is_key_released(KeyboardKey::KEY_Y)
    {
        state.pixels.set_enabled(true);
        //state.matrix.set_enabled(true);
        state.backlight.set_enabled(true);
        state.context.screen_enabled = true;
    }

    if state.context.screen_enabled
        && (input.is_key_pressed(KeyboardKey::KEY_Y)
            || state.context.time - input.last_activity_time() > 60.0)
    {
        state.pixels.set_enabled(false);
        //state.matrix.set_enabled(false);
        state.backlight.set_enabled(false);
        state.context.screen_enabled = false;
    }

    drop(input);

    state.pixels.update(&state.context, rl);
    //state.matrix.update(&state.context, rl, thread);

    state.macropad.update(&state.context, rl);
    state.thinkink.update(&state.context, rl);
    state.earth.update(&state.context, rl, thread);
}

#[no_mangle]
pub fn draw(
    state: &mut State,
    d: &mut raylib::drawing::RaylibDrawHandle,
    thread: &raylib::RaylibThread,
) {
    d.clear_background(Color::BLACK);

    state.pixels.draw(&state.context, d);
    state.macropad.draw(&state.context, d);
    state.thinkink.draw(&state.context, d, thread);
    //state.matrix.draw(&state.context, d, thread);
    state.earth.draw(&state.context, d, thread);

    let input = state.context.input.borrow();

    if input.is_key_down(KeyboardKey::KEY_ONE) {
        d.draw_rectangle_lines(0, 0, 240, 240, Color::ORANGE);
        d.draw_rectangle_lines(5, 5, 240 - 10, 240 - 10, Color::WHITE);
    }

    if input.is_key_pressed(KeyboardKey::KEY_ONE) {
        d.draw_rectangle_lines(10, 10, 240 - 20, 240 - 20, Color::RED);
    }

    if state.context.screen_enabled && input.is_key_down(KeyboardKey::KEY_THREE) {
        d.draw_fps(0, 0);
    }
}

#[no_mangle]
pub fn shutdown(state: State) {
    state.earth.shutdown();
}

#[cfg(feature = "reloader")]
#[no_mangle]
pub fn handle_reload(
    state: &mut State,
    rl: &mut raylib::RaylibHandle,
    thread: &raylib::RaylibThread,
) {
    state.earth.handle_reload(rl, thread);
    state.thinkink.handle_reload(rl, thread);
}
