use super::Context;
use base64::prelude::*;
use chrono::Datelike;
use chrono::Timelike;
use raylib::prelude::*;
use serialport::{available_ports, SerialPort, SerialPortInfo, SerialPortType};
use std::{env, io, str};

const IMAGE_WIDTH: u32 = 296;
const IMAGE_HEIGHT: u32 = 128;

pub struct ThinkInk {
    serial_port: Option<Box<dyn SerialPort>>,
    output_buffer: Vec<u8>,
    last_update: Option<f64>,
    light_spline: splines::Spline<f32, f32>,
    game_of_life: GameOfLife,

    font: Font,
    current_date: Option<chrono::DateTime<chrono::Local>>,
    current_date_string: Option<String>,
    last_date_string: Option<String>,

    preview_texture: Texture2D,
    preview_dithered_texture: Texture2D,
}

impl ThinkInk {
    pub fn new(rl: &mut raylib::RaylibHandle, thread: &raylib::RaylibThread) -> Self {
        let image = Image::gen_image_color(IMAGE_WIDTH as i32, IMAGE_HEIGHT as i32, Color::WHITE);

        Self {
            serial_port: None,
            output_buffer: Vec::new(),
            last_update: None,
            light_spline: create_spline(),
            game_of_life: GameOfLife::new(rl, thread),

            font: rl
                .load_font_from_memory(thread, ".ttf", FONT_DATA, 80, FontLoadEx::Default(0))
                .unwrap(),
            current_date: None,
            current_date_string: None,
            last_date_string: match std::fs::read("date.txt") {
                Ok(data) => Some(String::from_utf8(data).unwrap()),
                Err(_) => None,
            },

            preview_texture: rl.load_texture_from_image(thread, &image).unwrap(),
            preview_dithered_texture: rl.load_texture_from_image(thread, &image).unwrap(),
        }
    }

    pub fn open_serial(&mut self) {
        if self.serial_port.is_some() {
            return;
        }

        let matching_port_infos: Vec<SerialPortInfo> = available_ports()
            .unwrap()
            .into_iter()
            .filter(|port_info| match port_info.port_type {
                SerialPortType::UsbPort(ref usb_info) => {
                    return usb_info.vid == 0x239A && usb_info.pid == 0x80F1
                }
                _ => false,
            })
            .collect();

        for port_info in matching_port_infos {
            let mut port = match serialport::new(port_info.port_name.to_string(), 115200).open() {
                Ok(port) => port,
                Err(_) => continue,
            };

            let mut buffer = [0; 2];

            let read_count = match port.read(&mut buffer) {
                Ok(count) => count,
                Err(_) => continue,
            };

            if &buffer[0..read_count] != b"t\n" {
                continue;
            }

            self.serial_port = Some(port);
            log::debug!("= connected");
            break;
        }
    }

    pub fn update(&mut self, _context: &Context, rl: &RaylibHandle) {
        self.update_buffers();

        if self.last_update.is_none() || rl.get_time() - self.last_update.unwrap() > 60.0 {
            let now = chrono::Local::now();

            let target_value = self
                .light_spline
                .sample(to_minutes(now.hour(), now.minute()) as f32)
                .unwrap() as u32;

            self.send_message(json::object! {
                kind: "light",
                targetValue: target_value,
                speed: 100,
            });

            self.current_date = Some(now);
            self.current_date_string = Some(format!("{}", now.format("%m-%d")));

            self.last_update = Some(rl.get_time());
        }
    }

    fn update_buffers(&mut self) {
        let port = match self.serial_port.as_mut() {
            Some(port) => port,
            None => return,
        };

        let mut disconnected = false;

        let bytes_to_write = match port.bytes_to_write() {
            Ok(bytes) => bytes,
            Err(_) => {
                log::debug!("= error checking bytes to write");
                disconnected = true;
                0
            }
        };

        while self.output_buffer.len() > 0 && bytes_to_write < 256 && !disconnected {
            let output: Vec<u8> = self.output_buffer.iter().take(256).copied().collect();

            match port.write(&output) {
                Ok(write_count) => {
                    self.output_buffer.drain(0..write_count);
                }
                Err(e) => {
                    if e.kind() != io::ErrorKind::TimedOut {
                        log::debug!("= error writing");
                        disconnected = true;
                    }

                    break;
                }
            }
        }

        if disconnected {
            self.serial_port = None;
            log::debug!("= disconnected");
        }
    }

    fn send_message(&mut self, message: json::JsonValue) {
        log::debug!("-> {}", message.dump());
        self.output_buffer
            .extend_from_slice((message.dump() + "\n").as_bytes());
    }

    pub fn draw(&mut self, context: &Context, d: &mut RaylibDrawHandle, thread: &RaylibThread) {
        let input = context.input.borrow();

        if self.last_date_string.is_none()
            || self.last_date_string.as_ref().unwrap() != self.current_date_string.as_ref().unwrap()
            || input.is_key_pressed(KeyboardKey::KEY_TWO)
        {
            self.last_date_string = self.current_date_string.clone();
            std::fs::write("date.txt", self.last_date_string.as_ref().unwrap()).unwrap();

            let mut image = self.generate_image(d, thread);

            update_texture_with_image(&mut self.preview_texture, &image);

            image.color_grayscale();
            image.color_brightness(-30);
            image.dither(2, 2, 2, 2);

            self.send_dithered_image(&image);

            update_texture_with_image(
                &mut self.preview_dithered_texture,
                &convert_dithered_image(&image),
            );
        }

        #[cfg(not(feature = "pi"))]
        {
            d.draw_texture(&self.preview_texture, 260, 10, Color::WHITE);
            d.draw_rectangle(
                260,
                150,
                IMAGE_WIDTH as i32 + 4,
                IMAGE_HEIGHT as i32 + 4,
                Color::GRAY,
            );
            d.draw_texture(&self.preview_dithered_texture, 262, 152, Color::WHITE);
        }

        #[cfg(not(feature = "pi"))]
        {
            let mut points = Vec::new();

            for i in 0..1440 {
                points.push(Vector2::new(
                    i as f32 / 1440.0 * 600.0,
                    320.0 + 150.0 - (self.light_spline.sample(i as f32).unwrap() / 8190.0 * 150.0),
                ));
            }
            d.draw_line_strip(&points, Color::WHITE);
        }
    }

    fn generate_image(&mut self, d: &mut RaylibDrawHandle, thread: &RaylibThread) -> Image {
        let mut image =
            Image::gen_image_color(IMAGE_WIDTH as i32, IMAGE_HEIGHT as i32, Color::WHITE);

        let mut game_of_life_image = self.game_of_life.draw(d, thread);

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

        image.draw_rectangle(
            0,
            IMAGE_HEIGHT as i32 - 10,
            IMAGE_WIDTH as i32,
            10,
            Color::color_from_hsv(0.0, 0.0, 0.63),
        );
        image.draw_rectangle(
            0,
            IMAGE_HEIGHT as i32 - 10,
            (self.current_date.unwrap().ordinal() * IMAGE_WIDTH / 365) as i32,
            10,
            Color::BLACK,
        );

        let date_string = format!("{}", self.current_date.unwrap().format("%m-%d"));

        let size = measure_text_ex(&self.font, &date_string, 80.0, 0.0);

        image.draw_text_ex(
            &self.font,
            &date_string,
            Vector2::new(5.0, 138.0 - size.y - 10.0),
            80.0,
            0.0,
            Color::BLACK,
        );

        image
    }

    fn send_dithered_image(&mut self, image: &Image) {
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

        for (index, chunk_data) in data.as_slice().chunks(256).enumerate() {
            self.send_message(json::object! {
                kind: "displayData",
                offset: index * 256,
                data: BASE64_STANDARD.encode(chunk_data),
            });
        }

        self.send_message(json::object! { kind: "refreshDisplay", });
    }

    #[cfg(feature = "reloader")]
    pub fn handle_reload(&mut self, rl: &mut raylib::RaylibHandle, thread: &raylib::RaylibThread) {
        self.game_of_life.handle_reload(rl, thread);
        self.light_spline = create_spline();
    }
}

static GAME_OF_LIFE_SIZE: u32 = 296;

pub struct GameOfLife {
    shader: Shader,
    source_render_texture: RenderTexture2D,
    destination_render_texture: RenderTexture2D,
    seed_texture: Option<Texture2D>,
}

impl GameOfLife {
    pub fn new(rl: &mut raylib::RaylibHandle, thread: &raylib::RaylibThread) -> Self {
        let source_render_texture = rl
            .load_render_texture(thread, GAME_OF_LIFE_SIZE, GAME_OF_LIFE_SIZE)
            .unwrap();

        let destination_render_texture = rl
            .load_render_texture(thread, GAME_OF_LIFE_SIZE, GAME_OF_LIFE_SIZE)
            .unwrap();

        let image_path = "game_of_life.png";

        let mut image = if std::path::Path::new(image_path).exists() {
            let existing_image = Image::load_image(image_path).unwrap();
            log::debug!("loaded existing image");
            existing_image
        } else {
            let new_image = Image::gen_image_white_noise(
                GAME_OF_LIFE_SIZE as i32,
                GAME_OF_LIFE_SIZE as i32,
                0.5,
            );
            new_image.export_image("game_of_life_initial.png");
            log::debug!("created new image");
            new_image
        };

        image.flip_horizontal();
        image.rotate_cw();
        image.rotate_cw();

        Self {
            shader: load_game_of_life_shader(rl, thread),
            source_render_texture,
            destination_render_texture,
            seed_texture: Some(rl.load_texture_from_image(thread, &image).unwrap()),
        }
    }

    pub fn draw(&mut self, d: &mut RaylibDrawHandle, thread: &RaylibThread) -> Image {
        if self.seed_texture.is_some() {
            {
                let mut texture_mode =
                    d.begin_texture_mode(thread, &mut self.source_render_texture);
                texture_mode.draw_texture(self.seed_texture.as_ref().unwrap(), 0, 0, Color::WHITE);
            }
            self.seed_texture = None;
        }

        {
            let mut texture_mode =
                d.begin_texture_mode(thread, &mut self.destination_render_texture);
            let mut shader_mode = texture_mode.begin_shader_mode(&self.shader);

            shader_mode.draw_texture(&self.source_render_texture, 0, 0, Color::WHITE)
        }

        let image = self.destination_render_texture.get_texture_data().unwrap();

        std::mem::swap(
            &mut self.source_render_texture,
            &mut self.destination_render_texture,
        );

        image.export_image("game_of_life.png");

        image
    }

    #[cfg(feature = "reloader")]
    pub fn handle_reload(&mut self, rl: &mut raylib::RaylibHandle, thread: &raylib::RaylibThread) {
        self.shader = load_game_of_life_shader(rl, thread);
    }
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

fn convert_dithered_image(image: &Image) -> Image {
    let mut converted_image = Image::gen_image_color(image.width(), image.height(), Color::WHITE);

    let data = unsafe {
        std::slice::from_raw_parts(
            image.data as *const u8,
            image.width() as usize * image.height() as usize * 2,
        )
    };

    for x in 0..image.width() as i32 {
        for y in 0..image.height() as i32 {
            let index = ((y * image.width() + x) * 2) as usize;
            let value = ((data[index] & 0xf) as f32 / 15.0 * 255.0) as u8;
            let color = Color {
                r: value,
                g: value,
                b: value,
                a: 255,
            };

            converted_image.draw_pixel(x, y, color);
        }
    }

    converted_image
}

fn update_texture_with_image(texture: &mut Texture2D, image: &Image) {
    unsafe {
        texture.update_texture(std::slice::from_raw_parts(
            image.data as *const u8,
            image.get_pixel_data_size(),
        ));
    }
}

fn create_spline() -> splines::Spline<f32, f32> {
    splines::Spline::from_vec(vec![
        splines::Key::new(0.0, 0.0, splines::Interpolation::Step(1.0)),
        splines::Key::new(to_minutes(7, 0), 1000.0, splines::Interpolation::Cosine),
        splines::Key::new(to_minutes(8, 0), 8190.0, splines::Interpolation::Cosine),
        splines::Key::new(to_minutes(22, 0), 8190.0, splines::Interpolation::Cosine),
        splines::Key::new(to_minutes(24, 0), 1000.0, splines::Interpolation::default()),
    ])
}

fn to_minutes(hour: u32, minute: u32) -> f32 {
    hour as f32 * 60.0 + minute as f32
}
