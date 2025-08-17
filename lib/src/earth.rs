use super::Context;
use chrono::DurationRound;
use raylib::prelude::*;
use std::collections::VecDeque;
use std::fs;
use std::path::Path;
use std::sync::mpsc;
use std::thread;

pub struct Earth {
    earth_rotation_x: f32,
    earth_rotation_y: f32,
    model: Model,
    render_texture: RenderTexture2D,
    texture: Texture2D,
    images: VecDeque<EarthImage>,
    last_update: f64,
    image_value: f32,
    image_thread: std::thread::JoinHandle<()>,
    image_request_tx: Option<mpsc::SyncSender<u8>>,
    image_load_rx: mpsc::Receiver<EarthImage>,
}

struct EarthImage {
    date: chrono::DateTime<chrono::Utc>,
    image: Image,
}

unsafe impl Send for EarthImage {}

impl Earth {
    pub fn new(
        rl: &mut raylib::RaylibHandle,
        thread: &raylib::RaylibThread,
        api_client: std::sync::Arc<dyn super::ApiClient>,
    ) -> Self {
        let mesh = unsafe { Mesh::gen_mesh_sphere(thread, 1.0, 15, 15).make_weak() };
        let mut model = rl.load_model_from_mesh(thread, mesh.clone()).unwrap();

        let (image_thread, image_request_tx, image_load_rx) =
            start_image_thread(api_client.clone());

        image_request_tx.send(0).unwrap();

        let texture = rl
            .load_texture_from_image(&thread, &Image::gen_image_color(400, 800, Color::BLACK))
            .unwrap();

        model.materials_mut()[0].maps_mut()
            [raylib::consts::MaterialMapIndex::MATERIAL_MAP_ALBEDO as usize]
            .texture = *texture;

        Self {
            earth_rotation_x: 1.0,
            earth_rotation_y: -0.4,
            model,
            images: VecDeque::new(),
            texture,
            render_texture: rl.load_render_texture(thread, 240, 240).unwrap(),
            last_update: rl.get_time(),
            image_value: 0.0,
            image_thread,
            image_request_tx: Some(image_request_tx),
            image_load_rx,
        }
    }

    pub fn update(&mut self, context: &Context, rl: &mut RaylibHandle, _thread: &RaylibThread) {
        let input = context.input.borrow();

        if input.is_key_down(KeyboardKey::KEY_A) {
            self.earth_rotation_x += 2.0 * rl.get_frame_time();
        }

        if input.is_key_down(KeyboardKey::KEY_B) {
            self.earth_rotation_x += -2.0 * rl.get_frame_time();
        }

        if input.get_x_axis().abs() > 0.8 {
            self.earth_rotation_x += input.get_x_axis() * 2.0 * rl.get_frame_time();
        }

        if input.get_y_axis().abs() > 0.8 {
            self.earth_rotation_y += input.get_y_axis() * -2.0 * rl.get_frame_time();
        }

        if rl.get_time() - self.last_update > 60.0 {
            self.image_request_tx.as_mut().unwrap().send(0).unwrap();
            self.last_update = rl.get_time();
        }

        while let Ok(image) = self.image_load_rx.try_recv() {
            self.images.push_back(image);

            if self.images.len() > 144 {
                self.images.pop_front();
            }
        }

        if self.images.len() > 0 {
            let target_image_value = input.get_z_axis() * (self.images.len() - 1) as f32;

            self.image_value += (target_image_value - self.image_value) * 0.1;

            let image = &self.images[self.image_value as usize];

            unsafe {
                self.texture.update_texture(std::slice::from_raw_parts(
                    image.image.data as *const u8,
                    image.image.get_pixel_data_size(),
                ));
            }
        }
    }

    pub fn draw(&mut self, context: &Context, d: &mut RaylibDrawHandle, thread: &RaylibThread) {
        if context.screen_enabled {
            {
                let mut texture_mode = d.begin_texture_mode(thread, &mut self.render_texture);
                texture_mode.clear_background(Color::BLACK);

                let mut mode_3d = texture_mode.begin_mode3D(Camera3D::perspective(
                    Vector3::new(2.7, 0.0, 0.0),
                    Vector3::new(0.0, 0.0, 0.0),
                    Vector3::new(0.0, 0.0, -1.0),
                    45.0,
                ));

                let transform = Matrix::rotate_xyz(Vector3::new(0.0, 0.0, self.earth_rotation_x))
                    * Matrix::rotate_xyz(Vector3::new(0.0, self.earth_rotation_y, 0.0));

                self.model.set_transform(&transform);

                mode_3d.draw_model(&self.model, Vector3::new(0.0, 0.0, 0.0), 1.0, Color::WHITE);
            }

            d.draw_texture(&self.render_texture.texture(), 0, 0, Color::WHITE);
        }
    }

    pub fn shutdown(mut self) {
        self.image_request_tx = None;
        self.image_thread.join().unwrap();
    }

    #[cfg(feature = "reloader")]
    pub fn handle_reload(&mut self, rl: &mut raylib::RaylibHandle, thread: &raylib::RaylibThread) {
        self.image_request_tx.as_mut().unwrap().send(0).unwrap();
    }
}

fn start_image_thread(
    api_client: std::sync::Arc<dyn super::ApiClient>,
) -> (
    std::thread::JoinHandle<()>,
    mpsc::SyncSender<u8>,
    mpsc::Receiver<EarthImage>,
) {
    let (image_request_tx, image_request_rx) = mpsc::sync_channel::<u8>(100);
    let (image_load_tx, image_load_rx) = mpsc::sync_channel::<EarthImage>(100);

    let image_thread = thread::spawn(move || {
        let mut last_image_date: Option<chrono::DateTime<chrono::Utc>> = None;

        while let Ok(_message) = image_request_rx.recv() {
            let now = (chrono::offset::Utc::now() - chrono::Duration::hours(2))
                .duration_trunc(chrono::Duration::minutes(10))
                .unwrap();

            let mut current_date = last_image_date.unwrap_or(now - chrono::Duration::days(1));

            current_date += chrono::Duration::minutes(10);

            while current_date < now {
                let image = EarthImage {
                    date: current_date,
                    image: load_image(current_date, api_client.clone()),
                };

                last_image_date = Some(image.date.clone());
                image_load_tx.send(image).unwrap();

                current_date += chrono::Duration::minutes(10);
            }
        }
    });

    (image_thread, image_request_tx, image_load_rx)
}

fn load_image(
    date: chrono::DateTime<chrono::Utc>,
    api_client: std::sync::Arc<dyn super::ApiClient>,
) -> Image {
    fs::create_dir_all("geos-earth").unwrap();
    let image_path = format!("geos-earth/{}.png", date.format("%Y%m%dT%H%M%SZ"));

    if Path::new(&image_path).exists() {
        let mut existing_image = Image::load_image(&image_path).unwrap();
        log::debug!("loaded existing image {}", &image_path);
        existing_image.rotate_ccw();
        return existing_image;
    }

    let target_width = 800;
    let target_height = 400;

    let mut projected = Image::gen_image_color(target_width, target_height, Color::WHITE);

    let image = api_client
        .make_rammb_request("goes-18", date)
        .or_else(|_| { Ok::<Image, Box<dyn std::error::Error>>(Image::gen_image_color(700, 700, Color::WHITE)) })
        .unwrap();

    apply_image(&mut projected, &image, -180.0, -90.0, -136.9);

    let image = api_client
        .make_rammb_request("goes-19", date)
        .or_else(|_| { Ok::<Image, Box<dyn std::error::Error>>(Image::gen_image_color(700, 700, Color::WHITE)) })
        .unwrap();

    apply_image(&mut projected, &image, -120.0, -30.0, -75.2);

    let mut image = api_client
        .make_rammb_request("meteosat-0deg", date)
        .or_else(|_| { Ok::<Image, Box<dyn std::error::Error>>(Image::gen_image_color(700, 700, Color::WHITE)) })
        .unwrap();

    image.crop(Rectangle {
        x: 6.0,
        y: 6.0,
        width: image.width() as f32 - 12.0,
        height: image.height() as f32 - 12.0,
    });

    apply_image(&mut projected, &image, -50.0, 40.0, 0.0);

    let mut image = api_client
        .make_rammb_request("meteosat-9", date)
        .or_else(|_| { Ok::<Image, Box<dyn std::error::Error>>(Image::gen_image_color(700, 700, Color::WHITE)) })
        .unwrap();

    image.crop(Rectangle {
        x: 6.0,
        y: 6.0,
        width: image.width() as f32 - 12.0,
        height: image.height() as f32 - 12.0,
    });

    apply_image(&mut projected, &image, 20.0, 90.0, 45.5);

    let mut image = api_client
        .make_rammb_request("himawari", date)
        .or_else(|_| { Ok::<Image, Box<dyn std::error::Error>>(Image::gen_image_color(700, 700, Color::WHITE)) })
        .unwrap();

    image.crop(Rectangle {
        x: 6.0,
        y: 6.0,
        width: image.width() as f32 - 12.0,
        height: image.height() as f32 - 12.0,
    });

    apply_image(&mut projected, &image, 90.0, 180.0, 140.7);

    projected.export_image(&image_path);

    projected.rotate_ccw();
    projected
}

fn lat_to_pixel(projected: &Image, lat: f32) -> i32 {
    ((lat - 90.0) / 180.0 * -projected.height as f32).round() as i32
}

fn lng_to_pixel(projected: &Image, lng: f32) -> i32 {
    ((lng + 180.0) / 360.0 * projected.width as f32).round() as i32
}

fn apply_image(projected: &mut Image, source: &Image, lng_start: f32, lng_end: f32, sub_lng: f32) {
    let colors = source.get_image_data();

    let source_width = source.width;
    let source_height = source.height;

    for px in lng_to_pixel(&projected, lng_start)..lng_to_pixel(&projected, lng_end) {
        for py in lat_to_pixel(&projected, 70.0)..lat_to_pixel(&projected, -70.0) {
            let lng = px as f32 / projected.width as f32 * 360.0 - 180.0;
            let lat = -py as f32 / projected.height as f32 * 180.0 - 90.0;

            let c_lat = lat.to_radians().tan().atan();

            let r_l = 6378.137;

            let r_1 = 42164.0 - r_l * c_lat.cos() * (lng.to_radians() - sub_lng.to_radians()).cos();
            let r_2 = -r_l * c_lat.cos() * (lng.to_radians() - sub_lng.to_radians()).sin();
            let r_3 = r_l * c_lat.sin();
            let r_n = (r_1.powi(2) + r_2.powi(2) + r_3.powi(2)).sqrt();

            let x = (-r_2 / r_1).atan().to_degrees();
            let y = (-r_3 / r_n).asin().to_degrees();

            let scale = 17.343092;

            let source_x = (x / scale + 0.5) * source_width as f32;
            let source_y = (y / scale + 0.5) * source_height as f32;

            let lookup_x = source_x.round() as i32;
            let lookup_y = source_y.round() as i32;

            let source = if lookup_x >= 0
                && lookup_x < source_width
                && lookup_y >= 0
                && lookup_y < source_height
            {
                colors[(lookup_y * source_height + lookup_x) as usize]
            } else {
                Color::PINK
            };

            projected.draw_pixel(px, py, source)
        }
    }
}
