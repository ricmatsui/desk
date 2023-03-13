use raylib::prelude::*;

mod backlight;
mod earth;
mod input;
pub mod macropad;
mod pixels;
pub mod puck;
mod matrix;

pub trait ApiClient {
    fn make_noaa_tile_request(&self, level: u8, x: u8, y: u8) -> Image;
    fn make_noaa_archive_request(
        &self,
        width: u32,
        height: u32,
        date: chrono::DateTime<chrono::Utc>,
    ) -> Image;
    fn make_toggl_request(
        &self,
        method: &str,
        path: &str,
        body: Option<&json::JsonValue>,
    ) -> json::JsonValue;
    fn send_puck_image(&self, image: puck::PuckImage);
}

pub struct State {
    context: Context,
    pixels: pixels::Pixels,
    pub macropad: macropad::MacroPad,
    backlight: backlight::Backlight,
    puck: puck::Puck,
    earth: earth::Earth,
    matrix: matrix::Matrix,
}

pub struct Context {
    time: f64,
    input: input::Input,
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
    api_client: std::rc::Rc<dyn ApiClient>,
) -> State {
    State {
        context: Context {
            time: 0.0,
            input: input::init(),
            screen_enabled: false,
        },
        pixels: pixels::init(),
        macropad: macropad::init(api_client.clone()),
        backlight: backlight::init(),
        puck: puck::init(rl, thread, api_client.clone()),
        earth: earth::init(rl, thread, api_client.clone()),
        matrix: matrix::init(rl, thread, api_client.clone()),
    }
}

#[no_mangle]
pub fn update(state: &mut State, rl: &mut raylib::RaylibHandle, thread: &raylib::RaylibThread) {
    state.context.time = rl.get_time();
    input::update(&mut state.context.input, rl);

    if input::is_key_pressed(&state.context, KeyboardKey::KEY_X) {
        pixels::set_enabled(&mut state.pixels, true);
        matrix::set_enabled(&mut state.matrix, true);
        backlight::set_enabled(&mut state.backlight, true);
        state.context.screen_enabled = true;
    }

    if input::is_key_pressed(&state.context, KeyboardKey::KEY_Y) {
        pixels::set_enabled(&mut state.pixels, false);
        matrix::set_enabled(&mut state.matrix, false);
        backlight::set_enabled(&mut state.backlight, false);
        state.context.screen_enabled = false;
    }

    pixels::update(&mut state.pixels, &state.context, rl);
    macropad::update(&mut state.macropad, &state.context, rl);
    puck::update(&mut state.puck, &state.context, rl, thread);
    earth::update(&mut state.earth, &state.context, rl, thread);
    matrix::update(&mut state.matrix, &state.context, rl, thread);
}

#[no_mangle]
pub fn draw(
    state: &mut State,
    d: &mut raylib::drawing::RaylibDrawHandle,
    thread: &raylib::RaylibThread,
) {
    d.clear_background(Color::BLACK);

    pixels::draw(&state.pixels, &state.context, d);
    macropad::draw(&state.macropad, &state.context, d);
    puck::draw(&mut state.puck, &state.context, d, thread);
    earth::draw(&mut state.earth, &state.context, d, thread);
    matrix::draw(&mut state.matrix, &state.context, d, thread);

    if input::is_key_down(&state.context, KeyboardKey::KEY_ONE) {
        d.draw_rectangle_lines(0, 0, 240, 240, Color::ORANGE);
        d.draw_rectangle_lines(5, 5, 240 - 10, 240 - 10, Color::WHITE);
    }

    if input::is_key_pressed(&state.context, KeyboardKey::KEY_ONE) {
        d.draw_rectangle_lines(10, 10, 240 - 20, 240 - 20, Color::RED);
    }

    if state.context.screen_enabled && input::is_key_down(&state.context, KeyboardKey::KEY_THREE) {
        d.draw_fps(0, 0);
    }
}

#[cfg(feature = "reloader")]
#[no_mangle]
pub fn handle_reload(
    state: &mut State,
    rl: &mut raylib::RaylibHandle,
    thread: &raylib::RaylibThread,
) {
    puck::handle_reload(&mut state.puck, rl, thread);
}
