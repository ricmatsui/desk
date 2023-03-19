use super::{Context, I2cOperation};
use raylib::prelude::*;

const PIXEL_COUNT: usize = 28;

const LED_SHIM_ADDRESS: u16 = 0x75;

pub struct Pixels {
    pixels: [Color; PIXEL_COUNT],
    pixels_updated: bool,
    enabled: bool,
    brightness: f32,
    fade_speed: f32,
    fade_frequency: f32,
    api_client: std::rc::Rc<dyn super::ApiClient>,
    shim_enabled: bool,
}

pub fn init(api_client: std::rc::Rc<dyn super::ApiClient>) -> Pixels {
    api_client.enqueue_i2c(vec![
        I2cOperation::SetAddress(LED_SHIM_ADDRESS),
        I2cOperation::WriteByte(0xfd, 0x0b),
        I2cOperation::WriteByte(0x00, 0x00),
        I2cOperation::WriteByte(0x06, 0x00),
    ]);

    turn_on_shim(&api_client);

    api_client.enqueue_i2c(vec![
        I2cOperation::WriteByte(0xfd, 0x00),
        I2cOperation::WriteByte(0x00, 0x00),
        I2cOperation::WriteByte(0x01, 0xbf),
        I2cOperation::WriteByte(0x02, 0x3e),
        I2cOperation::WriteByte(0x03, 0x3e),
        I2cOperation::WriteByte(0x04, 0x3f),
        I2cOperation::WriteByte(0x05, 0xbe),
        I2cOperation::WriteByte(0x06, 0x07),
        I2cOperation::WriteByte(0x07, 0x86),
        I2cOperation::WriteByte(0x08, 0x30),
        I2cOperation::WriteByte(0x09, 0x30),
        I2cOperation::WriteByte(0x0a, 0x3f),
        I2cOperation::WriteByte(0x0b, 0xbe),
        I2cOperation::WriteByte(0x0c, 0x3f),
        I2cOperation::WriteByte(0x0d, 0xbe),
        I2cOperation::WriteByte(0x0e, 0x7f),
        I2cOperation::WriteByte(0x0f, 0xfe),
        I2cOperation::WriteByte(0x10, 0x7f),
        I2cOperation::WriteByte(0x11, 0x00),
    ]);

    for i in 0x24..0xB4 {
        api_client.enqueue_i2c(vec![I2cOperation::WriteByte(i, 0x00)])
    }

    turn_off_shim(&api_client);

    Pixels {
        pixels: [Color::BLACK; PIXEL_COUNT],
        pixels_updated: false,
        enabled: false,
        brightness: 1.0,
        fade_speed: 5.0,
        fade_frequency: 0.2,
        api_client,
        shim_enabled: false,
    }
}

pub fn update(pixels: &mut Pixels, context: &Context, _rl: &RaylibHandle) {
    if pixels.enabled {
        for i in 0..pixels.pixels.len() {
            set_pixel(
                pixels,
                i,
                Color::color_from_hsv(context.time as f32 * 90.0 + i as f32 * 2.0, 1.0, 1.0).fade(
                    ((context.time as f32 * pixels.fade_speed + i as f32 * pixels.fade_frequency)
                        .sin()
                        + 1.0)
                        / 2.0
                        * 0.15,
                ),
            );
        }
    }

    if pixels.pixels_updated {
        pixels.pixels_updated = false;

        if pixels.enabled && !pixels.shim_enabled {
            turn_on_shim(&pixels.api_client);
            pixels.shim_enabled = true;
        }

        if !pixels.enabled && pixels.shim_enabled {
            turn_off_shim(&pixels.api_client);
            pixels.shim_enabled = false;
        }

        send_shim_pixels(pixels);
    }
}

fn set_pixel(pixels: &mut Pixels, index: usize, color: Color) {
    pixels.pixels_updated = true;
    pixels.pixels[index] = color;
}

pub fn set_enabled(pixels: &mut Pixels, enabled: bool) {
    pixels.pixels_updated = true;
    pixels.enabled = enabled;
}

pub fn draw(pixels: &Pixels, _context: &Context, d: &mut RaylibDrawHandle) {
    #[cfg(not(feature = "pi"))]
    {
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
}

fn turn_on_shim(api_client: &std::rc::Rc<dyn super::ApiClient>) {
    api_client.enqueue_i2c(vec![
        I2cOperation::SetAddress(LED_SHIM_ADDRESS),
        I2cOperation::WriteByte(0xfd, 0x0b),
        I2cOperation::WriteByte(0x0a, 0x01),
        I2cOperation::WriteByte(0xfd, 0x00),
    ])
}

fn turn_off_shim(api_client: &std::rc::Rc<dyn super::ApiClient>) {
    api_client.enqueue_i2c(vec![I2cOperation::SetAddress(LED_SHIM_ADDRESS)]);

    for i in 0x24..0xB4 {
        api_client.enqueue_i2c(vec![I2cOperation::WriteByte(i, 0x00)]);
    }

    api_client.enqueue_i2c(vec![
        I2cOperation::WriteByte(0xfd, 0x0b),
        I2cOperation::WriteByte(0x0a, 0x00),
    ]);
}

fn send_shim_pixels(pixels: &mut Pixels) {
    let mut data: [u8; 145] = [0; 145];
    data[0] = 0x24;

    for i in 0..PIXEL_COUNT {
        data[1 + LED_OFFSET[i][0]] =
            LED_GAMMA[(pixels.pixels[i].r as f32 * pixels.pixels[i].a as f32 / 255.0
                * pixels.brightness) as usize];
        data[1 + LED_OFFSET[i][1]] =
            LED_GAMMA[(pixels.pixels[i].g as f32 * pixels.pixels[i].a as f32 / 255.0
                * pixels.brightness) as usize];
        data[1 + LED_OFFSET[i][2]] =
            LED_GAMMA[(pixels.pixels[i].b as f32 * pixels.pixels[i].a as f32 / 255.0
                * pixels.brightness) as usize];
    }

    pixels.api_client.enqueue_i2c(vec![
        I2cOperation::SetAddress(LED_SHIM_ADDRESS),
        I2cOperation::Write(data.to_vec()),
    ]);
}

const LED_OFFSET: [[usize; 3]; PIXEL_COUNT] = [
    [118, 69, 85],
    [117, 68, 101],
    [116, 84, 100],
    [115, 83, 99],
    [114, 82, 98],
    [113, 81, 97],
    [112, 80, 96],
    [134, 21, 37],
    [133, 20, 36],
    [132, 19, 35],
    [131, 18, 34],
    [130, 17, 50],
    [129, 33, 49],
    [128, 32, 48],
    [127, 47, 63],
    [121, 41, 57],
    [122, 25, 58],
    [123, 26, 42],
    [124, 27, 43],
    [125, 28, 44],
    [126, 29, 45],
    [15, 95, 111],
    [8, 89, 105],
    [9, 90, 106],
    [10, 91, 107],
    [11, 92, 108],
    [12, 76, 109],
    [13, 77, 93],
];

const LED_GAMMA: [u8; 256] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2,
    2, 2, 2, 3, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 6, 6, 6, 7, 7, 7, 8, 8, 8, 9, 9, 9, 10, 10, 11,
    11, 11, 12, 12, 13, 13, 13, 14, 14, 15, 15, 16, 16, 17, 17, 18, 18, 19, 19, 20, 21, 21, 22, 22,
    23, 23, 24, 25, 25, 26, 27, 27, 28, 29, 29, 30, 31, 31, 32, 33, 34, 34, 35, 36, 37, 37, 38, 39,
    40, 40, 41, 42, 43, 44, 45, 46, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61,
    62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 76, 77, 78, 79, 80, 81, 83, 84, 85, 86, 88,
    89, 90, 91, 93, 94, 95, 96, 98, 99, 100, 102, 103, 104, 106, 107, 109, 110, 111, 113, 114, 116,
    117, 119, 120, 121, 123, 124, 126, 128, 129, 131, 132, 134, 135, 137, 138, 140, 142, 143, 145,
    146, 148, 150, 151, 153, 155, 157, 158, 160, 162, 163, 165, 167, 169, 170, 172, 174, 176, 178,
    179, 181, 183, 185, 187, 189, 191, 193, 194, 196, 198, 200, 202, 204, 206, 208, 210, 212, 214,
    216, 218, 220, 222, 224, 227, 229, 231, 233, 235, 237, 239, 241, 244, 246, 248, 250, 252, 255,
];
