use raylib::prelude::*;

mod pixels;

pub struct State {
    context: Context,
    pixels: pixels::Pixels,
}

pub struct Context {
    time: f64,
}

#[no_mangle]
pub fn init() -> State {
    State {
        context: Context { time: 0.0 },
        pixels: pixels::init(),
    }
}

#[no_mangle]
pub fn update(state: &mut State, rl: &raylib::RaylibHandle) {
    state.context.time = rl.get_time();

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

    pixels::draw(&state.pixels, &state.context, d);
}
