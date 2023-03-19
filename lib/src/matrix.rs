use super::{Context, I2cOperation};
use raylib::prelude::*;

const MATRIX_ADDRESS: u16 = 0x30;

pub struct Matrix {
    image: Image,
    scroll_position: f32,
    updated: bool,
    enabled: bool,
    api_client: std::rc::Rc<dyn super::ApiClient>,
    driver_enabled: bool,
}

pub fn init(
    rl: &mut raylib::RaylibHandle,
    thread: &raylib::RaylibThread,
    api_client: std::rc::Rc<dyn super::ApiClient>,
) -> Matrix {
    let mut image = Image::gen_image_checked(13, 9, 1, 1, Color::BLACK, Color::WHITE);

    let mut scaling = [0x20; 181];
    scaling[0] = 0x00;

    let zero_pixels = [0x00; 181];

    api_client.enqueue_i2c(vec![
        I2cOperation::SetAddress(MATRIX_ADDRESS),
        // Reset
        I2cOperation::WriteByte(0xfe, 0xc5),
        I2cOperation::WriteByte(0xfd, 0x04),
        I2cOperation::WriteByte(0x3f, 0xae),
        I2cOperation::WriteByte(0x01, 0x03),
        // Set scaling
        I2cOperation::WriteByte(0xfe, 0xc5),
        I2cOperation::WriteByte(0xfd, 0x02),
        I2cOperation::Write(scaling.to_vec()),
        I2cOperation::WriteByte(0xfe, 0xc5),
        I2cOperation::WriteByte(0xfd, 0x03),
        I2cOperation::Write(scaling[..172].to_vec()),
        // Enable
        I2cOperation::WriteByte(0xfe, 0xc5),
        I2cOperation::WriteByte(0xfd, 0x04),
        I2cOperation::WriteByte(0x00, 0x01),
        // Clear
        I2cOperation::WriteByte(0xfe, 0xc5),
        I2cOperation::WriteByte(0xfd, 0x00),
        I2cOperation::Write(zero_pixels.to_vec()),
        I2cOperation::WriteByte(0xfe, 0xc5),
        I2cOperation::WriteByte(0xfd, 0x01),
        I2cOperation::Write(zero_pixels[..172].to_vec()),
        // Disable
        I2cOperation::WriteByte(0xfe, 0xc5),
        I2cOperation::WriteByte(0xfd, 0x04),
        I2cOperation::WriteByte(0x00, 0x00),
    ]);

    Matrix {
        image,
        scroll_position: 0.0,
        updated: false,
        enabled: false,
        driver_enabled: false,
        api_client,
    }
}

static TEXT: &str = "Hello World!";

pub fn update(
    matrix: &mut Matrix,
    context: &Context,
    rl: &mut RaylibHandle,
    thread: &RaylibThread,
) {
    if matrix.enabled {
        let text_width = measure_text(TEXT, 10);

        matrix.scroll_position -= 20.0 * rl.get_frame_time();
        if matrix.scroll_position < -text_width as f32 - 10.0 {
            matrix.scroll_position = 0.0;
        }

        matrix.image.draw_rectangle(0, 0, 13, 9, Color::BLACK);
        matrix.image.draw_text(
            TEXT,
            matrix.scroll_position as i32,
            0,
            10,
            Color::color_from_hsv(context.time as f32 * 90.0, 1.0, 1.0),
        );
        let text_width = measure_text(TEXT, 10);
        matrix.image.draw_text(
            TEXT,
            matrix.scroll_position as i32 + text_width + 10,
            0,
            10,
            Color::color_from_hsv(context.time as f32 * 90.0, 1.0, 1.0),
        );
        matrix.updated = true;
    }

    if matrix.updated {
        matrix.updated = false;

        if matrix.enabled && !matrix.driver_enabled {
            turn_on_matrix(matrix);
            matrix.driver_enabled = true;
        }

        if !matrix.enabled && matrix.driver_enabled {
            turn_off_matrix(matrix);
            matrix.driver_enabled = false;
        }

        send_matrix_pixels(matrix);
    }
}

pub fn draw(
    matrix: &mut Matrix,
    context: &Context,
    d: &mut RaylibDrawHandle,
    thread: &RaylibThread,
) {
    #[cfg(not(feature = "pi"))]
    {
        let data = matrix.image.get_image_data();
        d.draw_rectangle(575, 10, 13 * 9, 9 * 9, Color::DARKGRAY);
        for x in 0..13 {
            for y in 0..9 {
                d.draw_rectangle(575 + x * 9, 10 + y * 9, 6, 6, data[(y * 13 + x) as usize]);
            }
        }
    }
}

pub fn set_enabled(matrix: &mut Matrix, enabled: bool) {
    matrix.updated = true;
    matrix.enabled = enabled;
}

fn turn_on_matrix(matrix: &mut Matrix) {
    matrix.api_client.enqueue_i2c(vec![
        I2cOperation::SetAddress(MATRIX_ADDRESS),
        I2cOperation::WriteByte(0xfe, 0xc5),
        I2cOperation::WriteByte(0xfd, 0x04),
        I2cOperation::WriteByte(0x00, 0x01),
    ]);
}

fn turn_off_matrix(matrix: &mut Matrix) {
    let zero_pixels = [0x00; 181];

    matrix.api_client.enqueue_i2c(vec![
        I2cOperation::SetAddress(MATRIX_ADDRESS),
        // Clear
        I2cOperation::WriteByte(0xfe, 0xc5),
        I2cOperation::WriteByte(0xfd, 0x00),
        I2cOperation::Write(zero_pixels.to_vec()),
        I2cOperation::WriteByte(0xfe, 0xc5),
        I2cOperation::WriteByte(0xfd, 0x01),
        I2cOperation::Write(zero_pixels[..172].to_vec()),
        // Disable
        I2cOperation::WriteByte(0xfe, 0xc5),
        I2cOperation::WriteByte(0xfd, 0x04),
        I2cOperation::WriteByte(0x00, 0x00),
    ]);
}

fn send_matrix_pixels(matrix: &mut Matrix) {
    let image_data = matrix.image.get_image_data();

    let mut data: [u8; 352] = [0; 352];
    data[0] = 0x00;

    for x in 0..13 {
        for y in 0..9 {
            let flipped_x = 12 - x;
            let flipped_y = 8 - y;
            let (r_address, g_address, b_address) = get_pixel_addresses(x, y);
            let color = image_data[(flipped_y * 13 + flipped_x) as usize];
            data[1 + r_address] = LED_GAMMA[(color.r as f32 * color.a as f32 / 255.0) as usize];
            data[1 + g_address] = LED_GAMMA[(color.g as f32 * color.a as f32 / 255.0) as usize];
            data[1 + b_address] = LED_GAMMA[(color.b as f32 * color.a as f32 / 255.0) as usize];
        }
    }

    matrix.api_client.enqueue_i2c(vec![
        I2cOperation::SetAddress(MATRIX_ADDRESS),
        I2cOperation::WriteByte(0xfe, 0xc5),
        I2cOperation::WriteByte(0xfd, 0x00),
        I2cOperation::Write(data[..181].to_vec()),
    ]);
    data[180] = 0x00;
    matrix.api_client.enqueue_i2c(vec![
        I2cOperation::WriteByte(0xfe, 0xc5),
        I2cOperation::WriteByte(0xfd, 0x01),
        I2cOperation::Write(data[180..].to_vec()),
    ]);
}

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

static ROW_LOOKUP: [u16; 9] = [8, 5, 4, 3, 2, 1, 0, 7, 6];

fn get_pixel_addresses(x: u16, y: u16) -> (usize, usize, usize) {
    let row = ROW_LOOKUP[y as usize];

    let offset = 3
        * (if x < 10 {
            x + (row * 10)
        } else {
            x + (80 + row * 3)
        }) as usize;

    let (r_offset, g_offset, b_offset) = if x & 1 == 1 || x == 12 {
        (1, 0, 2)
    } else {
        (2, 1, 0)
    };

    (offset + r_offset, offset + g_offset, offset + b_offset)
}
