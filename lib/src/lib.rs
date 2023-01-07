use raylib::prelude::*;

mod backlight;
mod input;
pub mod macropad;
mod pixels;

pub trait ApiClient {
    fn make_toggl_request(
        &self,
        method: &str,
        path: &str,
        body: Option<&json::JsonValue>,
    ) -> json::JsonValue;
}

pub struct State {
    context: Context,
    pixels: pixels::Pixels,
    pub macropad: macropad::MacroPad,
    backlight: backlight::Backlight,
}

pub struct Context {
    time: f64,
    input: input::Input,
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
pub fn init(api_client: Box<dyn ApiClient>) -> State {
    State {
        context: Context {
            time: 0.0,
            input: input::init(),
        },
        pixels: pixels::init(),
        macropad: macropad::init(api_client),
        backlight: backlight::init(),
    }
}

#[no_mangle]
pub fn update(state: &mut State, rl: &raylib::RaylibHandle) {
    state.context.time = rl.get_time();
    input::update(&mut state.context.input, rl);

    pixels::update(&mut state.pixels, &state.context, rl);
    macropad::update(&mut state.macropad, &state.context, rl);

    if input::is_key_pressed(&state.context, KeyboardKey::KEY_X) {
        pixels::set_enabled(&mut state.pixels, true);
    }

    if input::is_key_pressed(&state.context, KeyboardKey::KEY_Y) {
        pixels::set_enabled(&mut state.pixels, false);
    }
}

#[no_mangle]
pub fn draw(state: &State, d: &mut raylib::drawing::RaylibDrawHandle) {
    d.clear_background(Color::WHITE);

    d.draw_text(
        &format!("Hello, World! {:.1}", state.context.time),
        12,
        12,
        50,
        Color::BLACK,
    );

    if input::is_key_down(&state.context, KeyboardKey::KEY_ONE) {
        d.draw_rectangle_lines(5, 5, 240 - 10, 240 - 10, Color::BLACK);
    }

    if input::is_key_pressed(&state.context, KeyboardKey::KEY_ONE) {
        d.draw_rectangle_lines(10, 10, 240 - 20, 240 - 20, Color::RED);
    }

    pixels::draw(&state.pixels, &state.context, d);
    macropad::draw(&state.macropad, &state.context, d);

    d.draw_fps(30, 30);
}
