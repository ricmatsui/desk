use raylib::prelude::*;

const GAME_OF_LIFE_SIZE: u32 = 296;

static IMAGE_PATH: &str = "game_of_life.png";

pub struct GameOfLife {
    shader: Shader,
    source_texture: Texture2D,
    destination_render_texture: RenderTexture2D,
}

impl GameOfLife {
    pub fn new(rl: &mut RaylibHandle, thread: &RaylibThread) -> Self {
        let mut image = if std::path::Path::new(IMAGE_PATH).exists() {
            let existing_image = Image::load_image(IMAGE_PATH).unwrap();
            tracing::info!("loaded existing image");
            existing_image
        } else {
            let new_image = Image::gen_image_white_noise(
                GAME_OF_LIFE_SIZE as i32,
                GAME_OF_LIFE_SIZE as i32,
                0.5,
            );
            new_image.export_image("game_of_life_initial.png");
            tracing::info!("created new image");
            new_image
        };

        image.flip_horizontal();
        image.rotate_cw();
        image.rotate_cw();

        let source_texture = rl.load_texture_from_image(thread, &image).unwrap();

        let destination_render_texture = rl
            .load_render_texture(thread, GAME_OF_LIFE_SIZE, GAME_OF_LIFE_SIZE)
            .unwrap();

        let mut shader = rl.load_shader_from_memory(
            thread,
            Some(str::from_utf8(GAME_OF_LIFE_SHADER_VS).unwrap()),
            Some(str::from_utf8(GAME_OF_LIFE_SHADER_FS).unwrap()),
        );

        shader.set_shader_value(
            shader.get_shader_location("pixelInverse"),
            1.0 / (GAME_OF_LIFE_SIZE as f32 + 0.5),
        );

        Self {
            shader,
            source_texture,
            destination_render_texture,
        }
    }

    pub fn draw_image(&mut self, d: &mut RaylibDrawHandle, thread: &RaylibThread) -> Image {
        {
            let mut texture_mode =
                d.begin_texture_mode(thread, &mut self.destination_render_texture);
            let mut shader_mode = texture_mode.begin_shader_mode(&self.shader);

            shader_mode.draw_texture(&self.source_texture, 0, 0, Color::WHITE)
        }

        let image = self.destination_render_texture.get_texture_data().unwrap();

        image.export_image(IMAGE_PATH);

        image
    }
}

#[cfg(not(feature = "pi"))]
static GAME_OF_LIFE_SHADER_VS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/shaders/build/330/game_of_life_shader_vs.vert"
));

#[cfg(not(feature = "pi"))]
static GAME_OF_LIFE_SHADER_FS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/shaders/build/330/game_of_life_shader_fs.frag"
));

#[cfg(feature = "pi")]
static GAME_OF_LIFE_SHADER_VS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/shaders/build/120/game_of_life_shader_vs.vert"
));

#[cfg(feature = "pi")]
static GAME_OF_LIFE_SHADER_FS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/shaders/build/120/game_of_life_shader_fs.frag"
));
