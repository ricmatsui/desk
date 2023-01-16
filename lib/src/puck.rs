use super::Context;
use raylib::prelude::*;
use std::path::Path;
use std::rc::Rc;
use std::{mem, str};

static GAME_OF_LIFE_SIZE: u32 = 296;

const IMAGE_WIDTH: u32 = 296;
const IMAGE_HEIGHT: u32 = 128;
const IMAGE_PIXEL_COUNT: u32 = IMAGE_WIDTH * IMAGE_HEIGHT;
const DITHERED_IMAGE_DATA_LENGTH: usize = IMAGE_PIXEL_COUNT as usize * 2;
const PUCK_IMAGE_DATA_LENGTH: usize = DITHERED_IMAGE_DATA_LENGTH / 8;

#[derive(Debug)]
pub struct PuckImage {
    pub data: [u8; PUCK_IMAGE_DATA_LENGTH],
}

pub struct Puck {
    game_of_life_shader: Shader,
    source_render_texture: RenderTexture2D,
    destination_render_texture: RenderTexture2D,
    seed_texture: Option<Texture2D>,

    api_client: Rc<dyn super::ApiClient>,

    font: Font,
    image_texture: Texture2D,
    puck_texture: Texture2D,
    current_date: Option<chrono::DateTime<chrono::Local>>,
    current_date_string: Option<String>,
    last_date_string: Option<String>,
    frame_time_since_last_check: Option<f32>,
}

pub fn init(
    rl: &mut RaylibHandle,
    thread: &RaylibThread,
    api_client: Rc<dyn super::ApiClient>,
) -> Puck {
    let source_render_texture = rl
        .load_render_texture(thread, GAME_OF_LIFE_SIZE, GAME_OF_LIFE_SIZE)
        .unwrap();

    let destination_render_texture = rl
        .load_render_texture(thread, GAME_OF_LIFE_SIZE, GAME_OF_LIFE_SIZE)
        .unwrap();

    let image_path = "game_of_life.png";

    let mut image = if Path::new(image_path).exists() {
        let existing_image = Image::load_image(image_path).unwrap();
        log::debug!("loaded existing image");
        existing_image
    } else {
        let new_image =
            Image::gen_image_white_noise(GAME_OF_LIFE_SIZE as i32, GAME_OF_LIFE_SIZE as i32, 0.5);
        new_image.export_image("game_of_life_initial.png");
        log::debug!("created new image");
        new_image
    };

    image.flip_horizontal();
    image.rotate_cw();
    image.rotate_cw();

    Puck {
        game_of_life_shader: load_game_of_life_shader(rl, thread),
        source_render_texture,
        destination_render_texture,
        seed_texture: Some(rl.load_texture_from_image(thread, &image).unwrap()),

        api_client,

        font: rl
            .load_font_from_memory(thread, ".ttf", FONT_DATA, 80)
            .unwrap(),
        image_texture: rl
            .load_texture_from_image(
                thread,
                &Image::gen_image_color(IMAGE_WIDTH as i32, IMAGE_HEIGHT as i32, Color::WHITE),
            )
            .unwrap(),
        puck_texture: rl
            .load_texture_from_image(
                thread,
                &Image::gen_image_color(IMAGE_WIDTH as i32, IMAGE_HEIGHT as i32, Color::WHITE),
            )
            .unwrap(),
        current_date: None,
        current_date_string: None,
        last_date_string: None,
        frame_time_since_last_check: None,
    }
}

pub fn update(puck: &mut Puck, context: &Context, rl: &mut RaylibHandle, thread: &RaylibThread) {
    if puck.frame_time_since_last_check.is_none()
        || puck.frame_time_since_last_check.unwrap() > 60.0
    {
        puck.current_date = Some(chrono::Local::now());
        puck.current_date_string = Some(format!("{}", puck.current_date.unwrap().format("%m-%d")));
    }

    puck.frame_time_since_last_check =
        Some(puck.frame_time_since_last_check.unwrap_or(0.0) + rl.get_frame_time());
}

pub fn draw(puck: &mut Puck, context: &Context, d: &mut RaylibDrawHandle, thread: &RaylibThread) {
    if puck.last_date_string.is_none()
        || puck.last_date_string.as_ref().unwrap() != puck.current_date_string.as_ref().unwrap()
        || super::input::is_key_pressed(context, KeyboardKey::KEY_TWO)
    {
        puck.last_date_string = puck.current_date_string.clone();

        let mut image = generate_image(puck, puck.current_date.unwrap(), d, thread);

        update_texture_with_image(&mut puck.image_texture, &image);

        image.color_grayscale();
        image.color_brightness(-30);

        let mut puck_image = image.clone();
        puck_image.rotate_ccw();
        puck_image.dither(2, 2, 2, 2);

        #[cfg(feature = "pi")]
        puck.api_client
            .send_puck_image(convert_to_puck_image(&puck_image));

        image.dither(2, 2, 2, 2);
        let converted_image = convert_puck_dithered_image(&image);

        update_texture_with_image(&mut puck.puck_texture, &converted_image);
    }

    #[cfg(not(feature = "pi"))]
    {
        d.draw_texture(&puck.image_texture, 260, 10, Color::WHITE);
        d.draw_rectangle(
            260,
            150,
            IMAGE_WIDTH as i32 + 4,
            IMAGE_HEIGHT as i32 + 4,
            Color::GRAY,
        );
        d.draw_texture(&puck.puck_texture, 262, 152, Color::WHITE);
    }
}

fn convert_to_puck_image(image: &Image) -> PuckImage {
    let mut transfer_data = [0; PUCK_IMAGE_DATA_LENGTH];

    let image_data =
        unsafe { std::slice::from_raw_parts(image.data as *const u8, DITHERED_IMAGE_DATA_LENGTH) };

    for i in 0..PUCK_IMAGE_DATA_LENGTH / 2 {
        let mut low_byte = 0;
        let mut high_byte = 0;

        for j in 0..8 {
            let pixel = (image_data[i * 16 + j * 2] & 0b1100) >> 2;

            low_byte |= ((pixel & 0b10) >> 1) << (7 - j);
            high_byte |= (pixel & 0b01) << (7 - j);
        }

        transfer_data[i] = low_byte;
        transfer_data[i + PUCK_IMAGE_DATA_LENGTH / 2] = high_byte;
    }

    return PuckImage {
        data: transfer_data,
    };
}

fn convert_puck_dithered_image(image: &Image) -> Image {
    let data =
        unsafe { std::slice::from_raw_parts(image.data as *const u8, DITHERED_IMAGE_DATA_LENGTH) };

    let mut dithered_image =
        Image::gen_image_color(IMAGE_WIDTH as i32, IMAGE_HEIGHT as i32, Color::WHITE);

    for x in 0..IMAGE_WIDTH as i32 {
        for y in 0..IMAGE_HEIGHT as i32 {
            let index = ((y * IMAGE_WIDTH as i32 + x) * 2) as usize;
            let value = ((data[index] & 0xf) as f32 / 15.0 * 255.0) as u8;
            let color = Color {
                r: value,
                g: value,
                b: value,
                a: 255,
            };

            dithered_image.draw_pixel(x, y, color);
        }
    }

    dithered_image
}

fn update_texture_with_image(texture: &mut Texture2D, image: &Image) {
    unsafe {
        texture.update_texture(std::slice::from_raw_parts(
            image.data as *const u8,
            image.get_pixel_data_size(),
        ));
    }
}

fn generate_image(
    puck: &mut Puck,
    current_date: chrono::DateTime<chrono::Local>,
    d: &mut RaylibDrawHandle,
    thread: &RaylibThread,
) -> Image {
    if puck.seed_texture.is_some() {
        {
            let mut texture_mode = d.begin_texture_mode(thread, &mut puck.source_render_texture);
            texture_mode.draw_texture(puck.seed_texture.as_ref().unwrap(), 0, 0, Color::WHITE);
        }
        puck.seed_texture = None;
    }

    {
        let mut texture_mode = d.begin_texture_mode(thread, &mut puck.destination_render_texture);
        let mut shader_mode = texture_mode.begin_shader_mode(&puck.game_of_life_shader);

        shader_mode.draw_texture(&puck.source_render_texture, 0, 0, Color::WHITE)
    }

    let mut render_image = puck.destination_render_texture.get_texture_data().unwrap();

    mem::swap(
        &mut puck.source_render_texture,
        &mut puck.destination_render_texture,
    );

    render_image.export_image("game_of_life.png");

    render_image.color_invert();
    render_image.color_brightness(30);

    let mut image = Image::gen_image_color(IMAGE_WIDTH as i32, IMAGE_HEIGHT as i32, Color::WHITE);

    let render_rectangle = Rectangle::new(
        0.0,
        0.0,
        render_image.width() as f32,
        render_image.height() as f32,
    );

    image.draw(
        &render_image,
        render_rectangle,
        render_rectangle,
        Color::WHITE,
    );
    image.draw_rectangle(
        0,
        IMAGE_HEIGHT as i32 - 10,
        IMAGE_WIDTH as i32,
        10,
        Color::color_from_hsv(0.0, 0.0, 0.63),
    );
    image.draw_rectangle(0, IMAGE_HEIGHT as i32 - 10, 50, 10, Color::BLACK);

    let date_string = format!("{}", current_date.format("%m-%d"));

    let size = measure_text_ex(&puck.font, &date_string, 80.0, 0.0);

    image.draw_text_ex(
        &puck.font,
        &date_string,
        Vector2::new(5.0, 138.0 - size.y - 10.0),
        80.0,
        0.0,
        Color::BLACK,
    );

    image
}

#[cfg(feature = "reloader")]
pub fn handle_reload(
    puck: &mut Puck,
    rl: &mut raylib::RaylibHandle,
    thread: &raylib::RaylibThread,
) {
    puck.game_of_life_shader = load_game_of_life_shader(rl, thread);
}

fn load_game_of_life_shader(rl: &mut RaylibHandle, thread: &RaylibThread) -> Shader {
    let mut shader = rl.load_shader_from_memory(
        thread,
        Some(str::from_utf8(GAME_OF_LIFE_SHADER_VS).unwrap()),
        Some(str::from_utf8(GAME_OF_LIFE_SHADER_FS).unwrap()),
    );

    shader.set_shader_value(
        shader.get_shader_location("pixelInverse"),
        1.0 / (GAME_OF_LIFE_SIZE as f32 + 0.5),
    );

    shader
}

#[cfg(not(feature = "pi"))]
static GAME_OF_LIFE_SHADER_VS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../shaders/build/330/game_of_life_shader_vs.vert"
));

#[cfg(not(feature = "pi"))]
static GAME_OF_LIFE_SHADER_FS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../shaders/build/330/game_of_life_shader_fs.frag"
));

#[cfg(feature = "pi")]
static GAME_OF_LIFE_SHADER_VS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../shaders/build/120/game_of_life_shader_vs.vert"
));

#[cfg(feature = "pi")]
static GAME_OF_LIFE_SHADER_FS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../shaders/build/120/game_of_life_shader_fs.frag"
));

static FONT_DATA: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../assets/KgHappy-wWZZ.ttf"
));
