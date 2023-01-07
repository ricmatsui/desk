use super::Context;
use raylib::prelude::*;
#[cfg(feature = "pi")]
use rppal::gpio::{Gpio, InputPin};
#[cfg(feature = "pi")]
use rppal::i2c::I2c;

pub struct Input {
    current: KeyState,
    previous: KeyState,

    #[cfg(feature = "pi")]
    i2c: I2c,
    #[cfg(feature = "pi")]
    pirate_buttons: PirateButtons,
}

#[cfg(feature = "pi")]
const BUTTON_SHIM_ADDRESS: u16 = 0x3f;

#[cfg(feature = "pi")]
pub struct PirateButtons {
    pin_a: InputPin,
    pin_b: InputPin,
    pin_x: InputPin,
    pin_y: InputPin,
}

#[derive(Copy, Clone)]
pub struct KeyState {
    key_one: bool,
    key_two: bool,
    key_three: bool,
    key_four: bool,
    key_five: bool,
    key_a: bool,
    key_b: bool,
    key_x: bool,
    key_y: bool,
}

pub fn init() -> Input {
    let initial_state = KeyState {
        key_one: false,
        key_two: false,
        key_three: false,
        key_four: false,
        key_five: false,
        key_a: false,
        key_b: false,
        key_x: false,
        key_y: false,
    };

    #[cfg(feature = "pi")]
    {
        let mut i2c = I2c::new().unwrap();
        let gpio = Gpio::new().unwrap();

        i2c.set_slave_address(BUTTON_SHIM_ADDRESS).unwrap();
        i2c.smbus_write_byte(0x03, 0x1f).unwrap();
        i2c.smbus_write_byte(0x02, 0x00).unwrap();
        i2c.smbus_write_byte(0x01, 0x00).unwrap();

        return Input {
            current: initial_state,
            previous: initial_state,

            i2c,
            pirate_buttons: PirateButtons {
                pin_a: gpio.get(5).unwrap().into_input_pullup(),
                pin_b: gpio.get(6).unwrap().into_input_pullup(),
                pin_x: gpio.get(16).unwrap().into_input_pullup(),
                pin_y: gpio.get(24).unwrap().into_input_pullup(),
            },
        };
    }

    #[cfg(not(feature = "pi"))]
    Input {
        current: initial_state,
        previous: initial_state,
    }
}

pub fn update(input: &mut Input, rl: &RaylibHandle) {
    input.previous = input.current;

    #[cfg(feature = "pi")]
    {
        input.current.key_a = input.pirate_buttons.pin_a.is_low();
        input.current.key_b = input.pirate_buttons.pin_b.is_low();
        input.current.key_x = input.pirate_buttons.pin_x.is_low();
        input.current.key_y = input.pirate_buttons.pin_y.is_low();

        input.i2c.set_slave_address(BUTTON_SHIM_ADDRESS).unwrap();
        let buttons = input.i2c.smbus_read_byte(0).unwrap();
        input.current.key_one =   buttons & 0b10000 == 0;
        input.current.key_two =   buttons & 0b01000 == 0;
        input.current.key_three = buttons & 0b00100 == 0;
        input.current.key_four =  buttons & 0b00010 == 0;
        input.current.key_five =  buttons & 0b00001 == 0;
    }

    #[cfg(not(feature = "pi"))]
    {
        input.current.key_one = rl.is_key_down(KeyboardKey::KEY_ONE);
        input.current.key_two = rl.is_key_down(KeyboardKey::KEY_TWO);
        input.current.key_three = rl.is_key_down(KeyboardKey::KEY_THREE);
        input.current.key_four = rl.is_key_down(KeyboardKey::KEY_FOUR);
        input.current.key_five = rl.is_key_down(KeyboardKey::KEY_FIVE);
        input.current.key_a = rl.is_key_down(KeyboardKey::KEY_A);
        input.current.key_b = rl.is_key_down(KeyboardKey::KEY_B);
        input.current.key_x = rl.is_key_down(KeyboardKey::KEY_X);
        input.current.key_y = rl.is_key_down(KeyboardKey::KEY_Y);
    }
}

pub fn is_key_pressed(context: &Context, key: KeyboardKey) -> bool {
    match key {
        KeyboardKey::KEY_ONE => context.input.current.key_one && !context.input.previous.key_one,
        KeyboardKey::KEY_TWO => context.input.current.key_two && !context.input.previous.key_two,
        KeyboardKey::KEY_THREE => {
            context.input.current.key_three && !context.input.previous.key_three
        }
        KeyboardKey::KEY_FOUR => context.input.current.key_four && !context.input.previous.key_four,
        KeyboardKey::KEY_FIVE => context.input.current.key_five && !context.input.previous.key_five,
        KeyboardKey::KEY_A => context.input.current.key_a && !context.input.previous.key_a,
        KeyboardKey::KEY_B => context.input.current.key_b && !context.input.previous.key_b,
        KeyboardKey::KEY_X => context.input.current.key_x && !context.input.previous.key_x,
        KeyboardKey::KEY_Y => context.input.current.key_y && !context.input.previous.key_y,
        _ => panic!("Unsupported keyboard key"),
    }
}

pub fn is_key_down(context: &Context, key: KeyboardKey) -> bool {
    match key {
        KeyboardKey::KEY_ONE => context.input.current.key_one,
        KeyboardKey::KEY_TWO => context.input.current.key_two,
        KeyboardKey::KEY_THREE => context.input.current.key_three,
        KeyboardKey::KEY_FOUR => context.input.current.key_four,
        KeyboardKey::KEY_FIVE => context.input.current.key_five,
        KeyboardKey::KEY_A => context.input.current.key_a,
        KeyboardKey::KEY_B => context.input.current.key_b,
        KeyboardKey::KEY_X => context.input.current.key_x,
        KeyboardKey::KEY_Y => context.input.current.key_y,
        _ => panic!("Unsupported keyboard key"),
    }
}
