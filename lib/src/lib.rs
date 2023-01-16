use raylib::prelude::*;

mod backlight;
mod input;
pub mod macropad;
mod pixels;
pub mod puck;

pub trait ApiClient {
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
    model: Model,
    texture: Texture2D,
    render_texture: RenderTexture2D,
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
    let mesh = unsafe { Mesh::gen_mesh_sphere(thread, 1.0, 6, 6).make_weak() };
    let mut model = rl.load_model_from_mesh(thread, mesh.clone()).unwrap();

    let checked = Image::gen_image_checked(20, 20, 1, 1, Color::RED, Color::GREEN);
    let texture = rl.load_texture_from_image(&thread, &checked).unwrap();

    model.materials_mut()[0].maps_mut()
        [raylib::consts::MaterialMapIndex::MATERIAL_MAP_ALBEDO as usize]
        .texture = *texture.as_ref();

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
        model,
        texture,
        render_texture: rl.load_render_texture(thread, 240, 240).unwrap(),
    }
}

#[no_mangle]
pub fn update(state: &mut State, rl: &mut raylib::RaylibHandle, thread: &raylib::RaylibThread) {
    state.context.time = rl.get_time();
    input::update(&mut state.context.input, rl);

    if input::is_key_pressed(&state.context, KeyboardKey::KEY_X) {
        pixels::set_enabled(&mut state.pixels, true);
        backlight::set_enabled(&mut state.backlight, true);
        state.context.screen_enabled = true;
    }

    if input::is_key_pressed(&state.context, KeyboardKey::KEY_Y) {
        pixels::set_enabled(&mut state.pixels, false);
        backlight::set_enabled(&mut state.backlight, false);
        state.context.screen_enabled = false;
    }

    pixels::update(&mut state.pixels, &state.context, rl);
    macropad::update(&mut state.macropad, &state.context, rl);
    puck::update(&mut state.puck, &state.context, rl, thread);
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

    if state.context.screen_enabled {
        {
            let mut texture_mode = d.begin_texture_mode(thread, &mut state.render_texture);
            texture_mode.clear_background(Color::BLACK);
            let mut camera_position = Vector3::new(0.0, 2.0, 5.0);
            camera_position.normalize();
            camera_position.scale(3.0);
            let mut mode_3d = texture_mode.begin_mode3D(Camera3D::perspective(
                camera_position,
                Vector3::new(0.0, 0.0, 0.0),
                Vector3::new(0.0, 1.0, 0.0),
                45.0,
            ));

            state.model.set_transform(&Matrix::rotate_xyz(Vector3::new(
                state.context.time as f32,
                state.context.time as f32 / 3.0,
                state.context.time as f32 / 5.0,
            )));

            mode_3d.draw_model(&state.model, Vector3::new(0.0, 0.0, 0.0), 1.0, Color::WHITE);
        }

        d.draw_texture(&state.render_texture.texture(), 0, 0, Color::WHITE);
        d.draw_fps(5, 5);
    }

    if input::is_key_down(&state.context, KeyboardKey::KEY_ONE) {
        d.draw_rectangle_lines(0, 0, 240, 240, Color::ORANGE);
        d.draw_rectangle_lines(5, 5, 240 - 10, 240 - 10, Color::WHITE);
    }

    if input::is_key_pressed(&state.context, KeyboardKey::KEY_ONE) {
        d.draw_rectangle_lines(10, 10, 240 - 20, 240 - 20, Color::RED);
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
