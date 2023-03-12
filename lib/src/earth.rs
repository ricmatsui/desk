use super::Context;
use chrono::Timelike;
use raylib::prelude::*;
use std::fs;
use std::path::Path;

pub struct Earth {
    ball_angular_velocity: f32,
    ball_rotation: f32,
    camera_z: f32,
    model: Model,
    render_texture: RenderTexture2D,
    textures: Vec<Texture2D>,
    timelapse_reset: f64,
}

pub fn init(
    rl: &mut raylib::RaylibHandle,
    thread: &raylib::RaylibThread,
    api_client: std::rc::Rc<dyn super::ApiClient>,
) -> Earth {
    let mesh = unsafe { Mesh::gen_mesh_sphere(thread, 1.0, 15, 15).make_weak() };
    let model = rl.load_model_from_mesh(thread, mesh.clone()).unwrap();

    let mut textures = Vec::new();

    for i in 0..30 {
        let date = chrono::Utc::now()
            .with_hour(0)
            .unwrap()
            .with_minute(0)
            .unwrap()
            .with_second(0)
            .unwrap()
            .with_nanosecond(0)
            .unwrap()
            - chrono::Duration::days(i + 2);

        fs::create_dir_all("earth").unwrap();
        let image_path = format!("earth/{}.png", date.format("%Y-%m-%d"));

        let mut image = if Path::new(&image_path).exists() {
            let existing_image = Image::load_image(&image_path).unwrap();
            log::debug!("loaded existing image {}", &image_path);
            existing_image
        } else {
            let mut image = Image::gen_image_color(800, 400, Color::WHITE);

            image.draw(
                &api_client.make_noaa_archive_request(800, 400, date),
                Rectangle::new(0.0, 0.0, 800.0, 400.0),
                Rectangle::new(0.0, 0.0, 800.0, 400.0),
                Color::WHITE,
            );

            image.export_image(&image_path);
            image
        };

        image.rotate_ccw();

        textures.push(rl.load_texture_from_image(&thread, &image).unwrap());
    }

    Earth {
        ball_angular_velocity: 0.0,
        ball_rotation: 0.0,
        camera_z: 1.86,
        model,
        textures,
        render_texture: rl.load_render_texture(thread, 240, 240).unwrap(),
        timelapse_reset: rl.get_time(),
    }
}

pub fn update(earth: &mut Earth, context: &Context, rl: &mut RaylibHandle, thread: &RaylibThread) {
    if super::input::is_key_down(&context, KeyboardKey::KEY_A) {
        earth.ball_angular_velocity += 0.1;
    } else {
        earth.ball_angular_velocity *= 0.99;
    }

    if earth.ball_angular_velocity < 0.1 {
        earth.ball_angular_velocity = 0.1;
    }

    earth.ball_rotation += -earth.ball_angular_velocity * rl.get_frame_time();

    if super::input::is_key_down(&context, KeyboardKey::KEY_B) {
        earth.timelapse_reset = context.time;
    }

    earth.model.materials_mut()[0].maps_mut()
        [raylib::consts::MaterialMapIndex::MATERIAL_MAP_ALBEDO as usize]
        .texture = *earth.textures[std::cmp::min(
        earth.textures.len() - 1,
        ((context.time - earth.timelapse_reset) * 3.0) as usize,
    )]
    .as_ref();

    if super::input::is_key_down(&context, KeyboardKey::KEY_FIVE) {
        earth.camera_z += 5.0 * rl.get_frame_time();
    }

    if super::input::is_key_down(&context, KeyboardKey::KEY_FOUR) {
        earth.camera_z -= 5.0 * rl.get_frame_time();
    }
}

pub fn draw(earth: &mut Earth, context: &Context, d: &mut RaylibDrawHandle, thread: &RaylibThread) {
    if context.screen_enabled {
        {
            let mut texture_mode = d.begin_texture_mode(thread, &mut earth.render_texture);
            texture_mode.clear_background(Color::BLACK);
            let mut mode_3d = texture_mode.begin_mode3D(Camera3D::perspective(
                Vector3::new(0.0, 2.0, earth.camera_z),
                Vector3::new(0.0, 0.0, 0.0),
                Vector3::new(0.0, 0.0, -1.0),
                45.0,
            ));

            earth.model.set_transform(&Matrix::rotate_xyz(Vector3::new(
                0.0,
                0.0,
                earth.ball_rotation,
            )));

            mode_3d.draw_model(&earth.model, Vector3::new(0.0, 0.0, 0.0), 1.0, Color::WHITE);
        }

        d.draw_texture(&earth.render_texture.texture(), 0, 0, Color::WHITE);
    }
}
