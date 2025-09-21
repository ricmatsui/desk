use crate::RaylibResponse;
use raylib::prelude::*;

mod game_of_life;
mod solar_system;
mod weather_forecast;

const IMAGE_WIDTH: u32 = 296;
const IMAGE_HEIGHT: u32 = 128;

pub fn thinkink_image(raylib_actor_transmit: &tokio::sync::mpsc::Sender<RaylibResponse>) {
    let current_date = chrono::Local::now();

    let (mut rl, thread) = raylib::init().size(240, 240).title("Desk").build();

    let font = rl
        .load_font_from_memory(&thread, ".ttf", FONT_DATA, 80, FontLoadEx::Default(0))
        .unwrap();

    let font_solid = rl
        .load_font_from_memory(&thread, ".ttf", FONT_SOLID_DATA, 30, FontLoadEx::Default(0))
        .unwrap();

    let mut game_of_life = game_of_life::GameOfLife::new(&mut rl, &thread);
    let mut solar_system = solar_system::SolarSystem::new(&mut rl, &thread);

    let mut d = rl.begin_drawing(&thread);

    let mut image = Image::gen_image_color(IMAGE_WIDTH as i32, IMAGE_HEIGHT as i32, Color::WHITE);

    let mut game_of_life_image = game_of_life.draw_image(&mut d, &thread);

    game_of_life_image.color_invert();
    game_of_life_image.color_brightness(30);

    let source_rectangle = Rectangle::new(
        0.0,
        0.0,
        game_of_life_image.width() as f32,
        game_of_life_image.height() as f32,
    );

    image.draw(
        &game_of_life_image,
        source_rectangle,
        source_rectangle,
        Color::WHITE,
    );

    if let Err(_) = weather_forecast::weather_forecast(&mut image, &font_solid) {
        image.draw_text_ex(
            &font_solid,
            "Weather error",
            Vector2::new(5.0, 5.0),
            30.0,
            0.0,
            Color::BLACK,
        );
    }

    let solar_system_image = solar_system.draw_image(&mut d, &thread);

    let source_rectangle = Rectangle::new(
        0.0,
        0.0,
        solar_system_image.width() as f32,
        solar_system_image.height() as f32,
    );

    let destination_rectangle = Rectangle::new(
        IMAGE_WIDTH as f32 - solar_system_image.width() as f32 - 14.0,
        IMAGE_HEIGHT as f32 / 2.0 - solar_system_image.height() as f32 / 2.0,
        solar_system_image.width() as f32,
        solar_system_image.height() as f32,
    );

    image.draw(
        &solar_system_image,
        source_rectangle,
        destination_rectangle,
        Color::WHITE,
    );

    let date_string = format!("{}", current_date.format("%m-%d"));

    let size = measure_text_ex(&font, &date_string, 80.0, 0.0);

    image.draw_text_ex(
        &font,
        &date_string,
        Vector2::new(5.0, 138.0 - size.y),
        80.0,
        0.0,
        Color::BLACK,
    );

    image.color_grayscale();
    image.color_brightness(-30);
    image.dither(2, 2, 2, 2);

    let mut data = vec![0; image.width() as usize * image.height() as usize / 4];

    let image_data = unsafe {
        std::slice::from_raw_parts(
            image.data as *const u8,
            image.width() as usize * image.height() as usize * 2,
        )
    };

    for i in 0..data.len() {
        let mut byte = 0;

        for j in 0..4 {
            let pixel = (image_data[i * 8 + j * 2] & 0b1100) >> 2;

            byte |= pixel << (3 - j) * 2;
        }

        data[i] = byte;
    }

    raylib_actor_transmit
        .blocking_send(RaylibResponse::ThinkInkImage(data))
        .unwrap();
}

static FONT_DATA: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/KgHappy-wWZZ.ttf"
));

static FONT_SOLID_DATA: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/KgHappy-solid.ttf"
));
