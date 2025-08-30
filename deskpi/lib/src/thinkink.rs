use super::{ApiClient, Context};
use base64::prelude::*;
use chrono::Datelike;
use chrono::Timelike;
use raylib::prelude::*;
use serialport::{available_ports, SerialPort, SerialPortInfo, SerialPortType};
use std::sync::Arc;
use std::{env, io, str};

const IMAGE_WIDTH: u32 = 296;
const IMAGE_HEIGHT: u32 = 128;

pub struct ThinkInk {
    serial_port: Option<Box<dyn SerialPort>>,
    output_buffer: Vec<u8>,
    api_client: Arc<dyn ApiClient>,

    last_update: Option<f64>,
    light_spline: splines::Spline<f32, f32>,
    servo_y_spline: splines::Spline<f32, f32>,
    game_of_life: GameOfLife,
    solar_system: SolarSystem,

    font: Font,
    font_solid: Font,
    current_date: Option<chrono::DateTime<chrono::Local>>,
    current_date_string: Option<String>,
    last_date_string: Option<String>,

    preview_texture: Texture2D,
    preview_dithered_texture: Texture2D,
}

impl ThinkInk {
    pub fn new(
        api_client: Arc<dyn ApiClient>,
        rl: &mut raylib::RaylibHandle,
        thread: &raylib::RaylibThread,
    ) -> Self {
        let image = Image::gen_image_color(IMAGE_WIDTH as i32, IMAGE_HEIGHT as i32, Color::WHITE);

        Self {
            serial_port: None,
            output_buffer: Vec::new(),
            api_client,
            last_update: None,
            light_spline: create_spline(),
            servo_y_spline: create_servo_y_spline(),
            game_of_life: GameOfLife::new(rl, thread),
            solar_system: SolarSystem::new(rl, thread),

            font: rl
                .load_font_from_memory(thread, ".ttf", FONT_DATA, 80, FontLoadEx::Default(0))
                .unwrap(),
            font_solid: rl
                .load_font_from_memory(thread, ".ttf", FONT_SOLID_DATA, 30, FontLoadEx::Default(0))
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

            std::thread::sleep(std::time::Duration::from_millis(1000));

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

        if self.last_update.is_none() || rl.get_time() - self.last_update.unwrap() > 1.0 {
            let now = chrono::Local::now();

            let target_value = self
                .light_spline
                .sample(to_seconds(now.hour(), now.minute(), now.second()) as f32)
                .unwrap() as u32;

            self.send_message(json::object! {
                kind: "light",
                targetValue: target_value,
                speed: 300,
            });

            let target_value = self
                .servo_y_spline
                .sample(to_seconds(now.hour(), now.minute(), now.second()) as f32)
                .unwrap() as u32;

            self.send_message(json::object! {
                kind: "servoY",
                targetValue: target_value,
                speed: 1,
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

    pub fn send_message(&mut self, message: json::JsonValue) {
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
            let mut image = self.generate_image(d, thread);

            update_texture_with_image(&mut self.preview_texture, &image);

            image.color_grayscale();
            image.color_brightness(-30);
            image.dither(2, 2, 2, 2);

            self.send_dithered_image(&image);

            self.last_date_string = self.current_date_string.clone();
            std::fs::write("date.txt", self.last_date_string.as_ref().unwrap()).unwrap();

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

        let response = self.api_client.make_open_meteo_request().unwrap();
        let forecast_length = response["hourly"]["temperature_2m"].len();

        let start_of_day = self
            .current_date
            .unwrap()
            .with_time(chrono::NaiveTime::MIN)
            .unwrap();
        let tomorrow = start_of_day + chrono::Duration::days(1);
        let mid_day = start_of_day + chrono::Duration::hours(12);
        let tomorrow_mid_day =
            start_of_day + chrono::Duration::days(1) + chrono::Duration::hours(12);

        let mut hot_times = vec![];
        let mut hot_start_index = None;

        let hot_start_threshold = 75.0;
        let hot_end_threshold = 72.0;

        let mut cold_times = vec![];
        let mut cold_start_index = None;

        let cold_start_threshold = 50.0;
        let cold_end_threshold = 55.0;

        for i in 0..forecast_length {
            let time = chrono::NaiveDateTime::parse_from_str(
                response["hourly"]["time"][i].as_str().unwrap(),
                "%Y-%m-%dT%H:%M",
            )
            .unwrap()
            .and_local_timezone(chrono::Local)
            .unwrap();

            if time < start_of_day {
                continue;
            }

            let current_temperature = response["hourly"]["temperature_2m"][i].as_f64().unwrap();

            if hot_start_index.is_none()
                && current_temperature > hot_start_threshold
                && time < tomorrow
            {
                hot_start_index = Some(i);
            }

            if hot_start_index.is_some()
                && (i == forecast_length - 1 || current_temperature < hot_end_threshold)
            {
                hot_times.push((
                    chrono::NaiveDateTime::parse_from_str(
                        response["hourly"]["time"][hot_start_index.unwrap()]
                            .as_str()
                            .unwrap(),
                        "%Y-%m-%dT%H:%M",
                    )
                    .unwrap(),
                    chrono::NaiveDateTime::parse_from_str(
                        response["hourly"]["time"][i].as_str().unwrap(),
                        "%Y-%m-%dT%H:%M",
                    )
                    .unwrap(),
                ));
                hot_start_index = None;
            }

            if cold_start_index.is_none()
                && current_temperature < cold_start_threshold
                && time > mid_day
                && time < tomorrow_mid_day
            {
                cold_start_index = Some(i);
            }

            if cold_start_index.is_some()
                && (i == forecast_length - 1 || current_temperature > cold_end_threshold)
            {
                cold_times.push((
                    chrono::NaiveDateTime::parse_from_str(
                        response["hourly"]["time"][cold_start_index.unwrap()]
                            .as_str()
                            .unwrap(),
                        "%Y-%m-%dT%H:%M",
                    )
                    .unwrap(),
                    chrono::NaiveDateTime::parse_from_str(
                        response["hourly"]["time"][i].as_str().unwrap(),
                        "%Y-%m-%dT%H:%M",
                    )
                    .unwrap(),
                ));
                cold_start_index = None;
            }
        }

        let hot_image = Image::load_image_from_mem(
            ".png",
            &HOT_IMAGE_DATA.to_vec(),
            HOT_IMAGE_DATA.len() as i32,
        )
        .unwrap();

        let cold_image = Image::load_image_from_mem(
            ".png",
            &COLD_IMAGE_DATA.to_vec(),
            COLD_IMAGE_DATA.len() as i32,
        )
        .unwrap();

        let mut y = 10;

        image.draw_rectangle(
            5,
            y - 5,
            100 + 10,
            25 * (hot_times.len() + cold_times.len()) as i32 + 10,
            Color::WHITE,
        );

        for (start, end) in hot_times {
            image.draw(
                &hot_image,
                Rectangle::new(0.0, 0.0, hot_image.width as f32, hot_image.height as f32),
                Rectangle::new(10.0, y as f32, 20.0, 20.0),
                Color::WHITE,
            );

            let start_string = start.format("%-I%P").to_string();
            let end_string = end.format("%-I%P").to_string();
            image.draw_text_ex(
                &self.font_solid,
                &format!(
                    "{}-{}",
                    start_string[..start_string.len() - 1].to_string(),
                    end_string[..end_string.len() - 1].to_string()
                ),
                Vector2::new(10.0 + 20.0 + 5.0, (y - 5) as f32),
                30.0,
                0.0,
                Color::BLACK,
            );

            y += 25;
        }

        for (start, end) in cold_times {
            image.draw(
                &cold_image,
                Rectangle::new(0.0, 0.0, cold_image.width as f32, cold_image.height as f32),
                Rectangle::new(10.0, y as f32, 20.0, 20.0),
                Color::WHITE,
            );

            let start_string = start.format("%-I%P").to_string();
            let end_string = end.format("%-I%P").to_string();
            image.draw_text_ex(
                &self.font_solid,
                &format!(
                    "{}-{}",
                    start_string[..start_string.len() - 1].to_string(),
                    end_string[..end_string.len() - 1].to_string()
                ),
                Vector2::new(10.0 + 20.0 + 5.0, (y - 5) as f32),
                20.0,
                0.0,
                Color::BLACK,
            );

            y += 25;
        }

        let solar_system_image = self.solar_system.draw(d, thread);

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

        let date_string = format!("{}", self.current_date.unwrap().format("%m-%d"));

        let size = measure_text_ex(&self.font, &date_string, 80.0, 0.0);

        image.draw_text_ex(
            &self.font,
            &date_string,
            Vector2::new(5.0, 138.0 - size.y),
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
        self.servo_y_spline = create_servo_y_spline();
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

pub struct SolarSystem {
    pub render_texture: RenderTexture2D,
    count: u32,
}

struct SolarSystemState {
    sun_position: Vector2,
    earth_position: Vector2,
    earth_longitude: f32,
    moon_position: Vector2,
    moon_geopoint: astro::coords::EclPoint,
    moon_closest_new_phase_direction: f64,
    moon_closest_full_phase_direction: f64,
    date: chrono::DateTime<chrono::Utc>,
}

impl SolarSystem {
    pub fn new(rl: &mut raylib::RaylibHandle, thread: &raylib::RaylibThread) -> Self {
        let render_texture = rl.load_render_texture(thread, 100, 100).unwrap();

        Self {
            render_texture,
            count: 0,
        }
    }

    fn calculate_state(date: chrono::DateTime<chrono::Utc>) -> SolarSystemState {
        let gregorian_date = astro::time::Date {
            year: date.year() as i16,
            month: date.month() as u8,
            decimal_day: astro::time::decimal_day(&astro::time::DayOfMonth {
                day: date.day() as u8,
                hr: date.hour() as u8,
                min: date.minute() as u8,
                sec: 0.0,
                time_zone: 0.0,
            }),
            cal_type: astro::time::CalType::Gregorian,
        };

        let julian_day = astro::time::julian_day(&gregorian_date);

        let (earth_longitude, _, earth_radius) =
            astro::planet::heliocent_coords(&astro::planet::Planet::Earth, julian_day);
        let earth_scale = 28.0;

        let (moon_geopoint, moon_radius) = astro::lunar::geocent_ecl_pos(julian_day);
        let moon_scale = 2.8 / 100000.0;

        let sun_position = Vector2::new(50.0, 50.0);

        let earth_position = sun_position
            + Vector2::new(
                (earth_longitude.cos() * earth_radius) as f32,
                (-earth_longitude.sin() * earth_radius) as f32,
            ) * earth_scale;

        let moon_position = earth_position
            + Vector2::new(
                (moon_geopoint.long.cos() * moon_radius) as f32,
                (-moon_geopoint.long.sin() * moon_radius) as f32,
            ) * moon_scale;

        let closest_new = astro::lunar::time_of_phase(&gregorian_date, &astro::lunar::Phase::New);
        let closest_full = astro::lunar::time_of_phase(&gregorian_date, &astro::lunar::Phase::Full);

        SolarSystemState {
            sun_position,
            earth_position,
            earth_longitude: earth_longitude as f32,
            moon_position,
            moon_geopoint,
            moon_closest_new_phase_direction: closest_new - julian_day,
            moon_closest_full_phase_direction: closest_full - julian_day,
            date,
        }
    }

    pub fn draw(&mut self, d: &mut RaylibDrawHandle, thread: &RaylibThread) -> Image {
        let current_date = chrono::Utc::now() + chrono::Duration::days(self.count as i64);

        let mut months = vec![];

        for i in 0..12 {
            months.push((
                i,
                SolarSystem::calculate_state(chrono::DateTime::from_naive_utc_and_offset(
                    current_date
                        .date_naive()
                        .with_day(1)
                        .unwrap()
                        .with_month(1 + i)
                        .unwrap()
                        .and_hms_opt(0, 0, 0)
                        .unwrap(),
                    chrono::Utc,
                )),
            ))
        }

        let mut moon_predictions = vec![];

        for i in 0..27 * 24 / 5 {
            moon_predictions.push(SolarSystem::calculate_state(
                current_date + chrono::Duration::hours(i as i64 * 5),
            ));
        }

        let mut moon_ecliptics = vec![];
        let mut lunar_eclipses = vec![];
        let mut solar_eclipses = vec![];

        for window in moon_predictions.windows(2) {
            if let [previous, current] = window {
                if previous.moon_geopoint.lat.signum() != current.moon_geopoint.lat.signum() {
                    moon_ecliptics.push(previous);

                    if previous.moon_closest_full_phase_direction.abs() < 1.0 {
                        lunar_eclipses.push(previous);
                    }

                    if previous.moon_closest_new_phase_direction.abs() < 1.0 {
                        solar_eclipses.push(previous);
                    }
                }
            }
        }

        let mut earth_history = vec![];

        for i in 0..15 {
            earth_history.push(SolarSystem::calculate_state(
                current_date - chrono::Duration::days(i as i64 * 4),
            ))
        }

        let mut moon_history = vec![];

        for i in 0..8 {
            moon_history.push(SolarSystem::calculate_state(
                current_date - chrono::Duration::hours(i as i64 * 24),
            ))
        }

        let state = SolarSystem::calculate_state(current_date);

        {
            let mut d = d.begin_texture_mode(thread, &mut self.render_texture);

            log::debug!("earth: {:?}", state.earth_position);
            let mut d = d.begin_mode2D(Camera2D {
                rotation: 0.0,
                target: state.earth_position,
                offset: Vector2::new(50.0, 50.0),
                zoom: 2.0,
            });

            d.clear_background(Color::WHITE);

            for moon_ecliptic in moon_ecliptics {
                let point = state.earth_position + moon_ecliptic.moon_position
                    - moon_ecliptic.earth_position;

                if moon_ecliptic.date - state.date > chrono::Duration::days(20) {
                    continue;
                }

                d.draw_circle_v(point, 1.0, Color::color_from_hsv(0.0, 0.0, 0.38));
            }

            for lunar_eclipse in lunar_eclipses.iter() {
                let point = state.earth_position + lunar_eclipse.moon_position
                    - lunar_eclipse.earth_position;

                d.draw_circle_v(point, 2.8, Color::color_from_hsv(0.0, 0.0, 0.38));
            }

            for solar_eclipse in solar_eclipses.iter() {
                let point = state.earth_position + solar_eclipse.moon_position
                    - solar_eclipse.earth_position;

                d.draw_circle_v(point, 2.8, Color::color_from_hsv(0.0, 0.0, 0.38));
            }

            for (index, state) in months {
                let earth_position =
                    Vector2::new(state.earth_longitude.cos(), -state.earth_longitude.sin());

                if index % 3 == 0 {
                    d.draw_poly(
                        state.sun_position + earth_position * 49.0,
                        3,
                        7.0,
                        -state.earth_longitude.to_degrees() - 30.0,
                        Color::color_from_hsv(0.0, 0.0, 0.38),
                    );
                } else {
                    d.draw_line_ex(
                        state.sun_position + earth_position * 45.0,
                        state.sun_position + earth_position * 55.0,
                        2.0,
                        Color::color_from_hsv(0.0, 0.0, 0.38),
                    );
                }
            }

            for (i, window) in earth_history.windows(2).enumerate() {
                if let [previous, current] = window {
                    d.draw_line_ex(
                        previous.earth_position,
                        current.earth_position,
                        3.0,
                        Color::color_from_hsv(0.0, 0.0, 0.38)
                            .fade(1.0 - i as f32 / earth_history.len() as f32),
                    );
                }
            }

            d.draw_circle_v(state.moon_position, 5.0, Color::WHITE);

            for (i, window) in moon_history.windows(2).enumerate() {
                if let [previous, current] = window {
                    d.draw_line_ex(
                        state.earth_position + previous.moon_position - previous.earth_position,
                        state.earth_position + current.moon_position - current.earth_position,
                        3.0,
                        Color::color_from_hsv(0.0, 0.0, 0.38)
                            .fade(1.0 - i as f32 / moon_history.len() as f32),
                    );
                }
            }

            d.draw_circle_v(state.sun_position, 10.0, Color::BLACK);

            d.draw_circle_v(state.earth_position, 4.5, Color::BLACK);

            d.draw_circle_v(state.moon_position, 2.8, Color::BLACK);

            for lunar_eclipse in lunar_eclipses.iter() {
                let point = state.earth_position + lunar_eclipse.moon_position
                    - lunar_eclipse.earth_position;

                d.draw_circle_v(point, 1.8, Color::WHITE);
            }

            for solar_eclipse in solar_eclipses {
                let point = state.earth_position + solar_eclipse.moon_position
                    - solar_eclipse.earth_position;

                d.draw_circle_v(point, 1.8, Color::WHITE);
                d.draw_circle_v(point, 1.2, Color::BLACK);
            }
        }

        let mut image = self.render_texture.get_texture_data().unwrap();
        image.flip_vertical();

        {
            let mut d = d.begin_texture_mode(thread, &mut self.render_texture);

            d.clear_background(Color::BLACK);
            d.draw_circle_v(Vector2::new(50.0, 50.0), 50.0, Color::WHITE);
        }

        let mut mask = self.render_texture.get_texture_data().unwrap();
        mask.flip_vertical();

        image.alpha_mask(&mask);

        {
            let mut d = d.begin_texture_mode(thread, &mut self.render_texture);

            d.clear_background(Color::BLANK);
            d.draw_ring(state.sun_position, 48.5, 50.0, 0.0, 360.0, 40, Color::BLACK);
        }

        let mut ring = self.render_texture.get_texture_data().unwrap();
        ring.flip_vertical();

        image.draw(
            &ring,
            Rectangle::new(0.0, 0.0, ring.width() as f32, ring.height() as f32),
            Rectangle::new(0.0, 0.0, ring.width() as f32, ring.height() as f32),
            Color::WHITE,
        );

        image
    }
}

static FONT_DATA: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../assets/KgHappy-wWZZ.ttf"
));

static FONT_SOLID_DATA: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../assets/KgHappy-solid.ttf"
));

static HOT_IMAGE_DATA: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../assets/heat-wave.png"
));

static COLD_IMAGE_DATA: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../assets/thermometer-minus.png"
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
        splines::Key::new(to_seconds(6, 0, 0), 0.0, splines::Interpolation::Cosine),
        splines::Key::new(to_seconds(6, 1, 0), 4096.0, splines::Interpolation::Cosine),
        splines::Key::new(to_seconds(7, 0, 0), 4096.0, splines::Interpolation::Cosine),
        splines::Key::new(to_seconds(7, 1, 0), 8190.0, splines::Interpolation::Cosine),
        splines::Key::new(to_seconds(21, 0, 0), 8190.0, splines::Interpolation::Cosine),
        splines::Key::new(to_seconds(21, 1, 0), 4096.0, splines::Interpolation::Cosine),
        splines::Key::new(
            to_seconds(23, 20, 0),
            4096.0,
            splines::Interpolation::Cosine,
        ),
        splines::Key::new(to_seconds(23, 21, 0), 0.0, splines::Interpolation::Cosine),
        splines::Key::new(to_seconds(24, 0, 0), 0.0, splines::Interpolation::default()),
    ])
}

fn create_servo_y_spline() -> splines::Spline<f32, f32> {
    splines::Spline::from_vec(vec![
        splines::Key::new(0.0, 400.0, splines::Interpolation::Step(1.0)),
        splines::Key::new(to_seconds(6, 0, 0), 400.0, splines::Interpolation::Cosine),
        splines::Key::new(to_seconds(6, 1, 0), 200.0, splines::Interpolation::Cosine),
        splines::Key::new(to_seconds(23, 20, 0), 200.0, splines::Interpolation::Cosine),
        splines::Key::new(to_seconds(23, 21, 0), 400.0, splines::Interpolation::Cosine),
        splines::Key::new(
            to_seconds(24, 0, 0),
            400.0,
            splines::Interpolation::default(),
        ),
    ])
}

fn to_seconds(hour: u32, minute: u32, second: u32) -> f32 {
    hour as f32 * 3600.0 + minute as f32 * 60.0 + second as f32
}
