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

impl Input {
    pub fn new() -> Self {
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

            return Self {
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
        Self {
            current: initial_state,
            previous: initial_state,
        }
    }

    pub fn update(&mut self, rl: &RaylibHandle) {
        self.previous = self.current;

        #[cfg(feature = "pi")]
        {
            self.current.key_a = self.pirate_buttons.pin_a.is_low();
            self.current.key_b = self.pirate_buttons.pin_b.is_low();
            self.current.key_x = self.pirate_buttons.pin_x.is_low();
            self.current.key_y = self.pirate_buttons.pin_y.is_low();

            self.i2c.set_slave_address(BUTTON_SHIM_ADDRESS).unwrap();
            let buttons = self.i2c.smbus_read_byte(0).unwrap();
            self.current.key_one = buttons & 0b10000 == 0;
            self.current.key_two = buttons & 0b01000 == 0;
            self.current.key_three = buttons & 0b00100 == 0;
            self.current.key_four = buttons & 0b00010 == 0;
            self.current.key_five = buttons & 0b00001 == 0;
        }

        #[cfg(not(feature = "pi"))]
        {
            self.current.key_one = rl.is_key_down(KeyboardKey::KEY_ONE);
            self.current.key_two = rl.is_key_down(KeyboardKey::KEY_TWO);
            self.current.key_three = rl.is_key_down(KeyboardKey::KEY_THREE);
            self.current.key_four = rl.is_key_down(KeyboardKey::KEY_FOUR);
            self.current.key_five = rl.is_key_down(KeyboardKey::KEY_FIVE);
            self.current.key_a = rl.is_key_down(KeyboardKey::KEY_A);
            self.current.key_b = rl.is_key_down(KeyboardKey::KEY_B);
            self.current.key_x = rl.is_key_down(KeyboardKey::KEY_X);
            self.current.key_y = rl.is_key_down(KeyboardKey::KEY_Y);
        }
    }

    pub fn is_key_pressed(&self, key: KeyboardKey) -> bool {
        match key {
            KeyboardKey::KEY_ONE => {
                self.current.key_one && !self.previous.key_one
            }
            KeyboardKey::KEY_TWO => {
                self.current.key_two && !self.previous.key_two
            }
            KeyboardKey::KEY_THREE => {
                self.current.key_three && !self.previous.key_three
            }
            KeyboardKey::KEY_FOUR => {
                self.current.key_four && !self.previous.key_four
            }
            KeyboardKey::KEY_FIVE => {
                self.current.key_five && !self.previous.key_five
            }
            KeyboardKey::KEY_A => self.current.key_a && !self.previous.key_a,
            KeyboardKey::KEY_B => self.current.key_b && !self.previous.key_b,
            KeyboardKey::KEY_X => self.current.key_x && !self.previous.key_x,
            KeyboardKey::KEY_Y => self.current.key_y && !self.previous.key_y,
            _ => panic!("Unsupported keyboard key"),
        }
    }

    pub fn is_key_released(&self, key: KeyboardKey) -> bool {
        match key {
            KeyboardKey::KEY_ONE => {
                !self.current.key_one && self.previous.key_one
            }
            KeyboardKey::KEY_TWO => {
                !self.current.key_two && self.previous.key_two
            }
            KeyboardKey::KEY_THREE => {
                !self.current.key_three && self.previous.key_three
            }
            KeyboardKey::KEY_FOUR => {
                !self.current.key_four && self.previous.key_four
            }
            KeyboardKey::KEY_FIVE => {
                !self.current.key_five && self.previous.key_five
            }
            KeyboardKey::KEY_A => !self.current.key_a && self.previous.key_a,
            KeyboardKey::KEY_B => !self.current.key_b && self.previous.key_b,
            KeyboardKey::KEY_X => !self.current.key_x && self.previous.key_x,
            KeyboardKey::KEY_Y => !self.current.key_y && self.previous.key_y,
            _ => panic!("Unsupported keyboard key"),
        }
    }

    pub fn is_key_down(&self, key: KeyboardKey) -> bool {
        match key {
            KeyboardKey::KEY_ONE => self.current.key_one,
            KeyboardKey::KEY_TWO => self.current.key_two,
            KeyboardKey::KEY_THREE => self.current.key_three,
            KeyboardKey::KEY_FOUR => self.current.key_four,
            KeyboardKey::KEY_FIVE => self.current.key_five,
            KeyboardKey::KEY_A => self.current.key_a,
            KeyboardKey::KEY_B => self.current.key_b,
            KeyboardKey::KEY_X => self.current.key_x,
            KeyboardKey::KEY_Y => self.current.key_y,
            _ => panic!("Unsupported keyboard key"),
        }
    }
}
