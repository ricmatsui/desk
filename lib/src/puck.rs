use super::Context;
use raylib::prelude::*;
use std::{mem, str};

static GAME_OF_LIFE_SIZE: u32 = 296;

pub struct Puck {
    game_of_life_shader: Shader,
    source_render_texture: RenderTexture2D,
    destination_render_texture: RenderTexture2D,
    seed_texture: Option<Texture2D>,
}

pub fn init(rl: &mut RaylibHandle, thread: &RaylibThread) -> Puck {
    let source_render_texture = rl
        .load_render_texture(thread, GAME_OF_LIFE_SIZE, GAME_OF_LIFE_SIZE)
        .unwrap();

    let destination_render_texture = rl
        .load_render_texture(thread, GAME_OF_LIFE_SIZE, GAME_OF_LIFE_SIZE)
        .unwrap();

    let image =
        Image::gen_image_white_noise(GAME_OF_LIFE_SIZE as i32, GAME_OF_LIFE_SIZE as i32, 0.5);

    Puck {
        game_of_life_shader: load_game_of_life_shader(rl, thread),
        source_render_texture,
        destination_render_texture,
        seed_texture: Some(rl.load_texture_from_image(thread, &image).unwrap()),
    }
}

pub fn update(puck: &mut Puck, context: &Context, rl: &mut RaylibHandle, thread: &RaylibThread) {
    if !context.screen_enabled {
        return;
    }

    mem::swap(
        &mut puck.source_render_texture,
        &mut puck.destination_render_texture,
    );
}

pub fn draw(puck: &mut Puck, context: &Context, d: &mut RaylibDrawHandle, thread: &RaylibThread) {
    if !context.screen_enabled {
        return;
    }

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

    d.draw_texture_ex(
        &puck.destination_render_texture,
        Vector2::new(0.0, 0.0),
        0.0,
        1.0,
        Color::WHITE,
    );
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
    "/../shaders/build/100/game_of_life_shader_vs.vert"
));

#[cfg(feature = "pi")]
static GAME_OF_LIFE_SHADER_FS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../shaders/build/100/game_of_life_shader_fs.frag"
));
