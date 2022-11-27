use raylib::prelude::*;
use super::Context;

const PIXEL_COUNT: usize = 28;

pub struct Pixels {
    pixels: [Color; PIXEL_COUNT],
    pixels_updated: bool,
    brightness: f32,
    fade_speed: f32,
    fade_frequency: f32,
}

pub fn init() -> Pixels {
    Pixels {
        pixels: [Color::GREEN; PIXEL_COUNT],
        pixels_updated: false,
        brightness: 1.0,
        fade_speed: 5.0,
        fade_frequency: 0.2,
    }
}

pub fn update(pixels: &mut Pixels, context: &Context, _rl: &RaylibHandle) {
    for i in 0..pixels.pixels.len() {
        set_pixel(
            pixels,
            i,
            Color::color_from_hsv(context.time as f32 * 90.0 + i as f32 * 2.0, 1.0, 1.0).fade(
                ((context.time as f32 * pixels.fade_speed + i as f32 * pixels.fade_frequency)
                    .sin()
                    + 1.0)
                    / 2.0
                    * pixels.brightness,
            ),
        );
    }
}

fn set_pixel(pixels: &mut Pixels, index: usize, color: Color) {
    pixels.pixels_updated = true;
    pixels.pixels[index] = color;
}

pub fn draw(pixels: &Pixels, _context: &Context, d: &mut RaylibDrawHandle) {
    d.draw_rectangle(10, 300, 12 * pixels.pixels.len() as i32, 8, Color::BLACK);

    for i in 0..pixels.pixels.len() {
        d.draw_rectangle(
            10 + i as i32 * 12,
            300,
            8,
            8,
            pixels.pixels[i].fade(pixels.pixels[i].a as f32 / 255.0 * 10.0 * pixels.brightness),
        );
    }
}
