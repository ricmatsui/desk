use raylib::prelude::*;

mod input;
mod pixels;

pub struct State {
    context: Context,
    pixels: pixels::Pixels,
}

pub struct Context {
    time: f64,
    input: input::Input,
}

#[no_mangle]
pub fn init() -> State {
    State {
        context: Context {
            time: 0.0,
            input: input::init(),
        },
        pixels: pixels::init(),
    }
}

#[no_mangle]
pub fn update(state: &mut State, rl: &raylib::RaylibHandle) {
    state.context.time = rl.get_time();
    input::update(&mut state.context.input, rl);

    pixels::update(&mut state.pixels, &state.context, rl);
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

    d.draw_fps(30, 30);
}
