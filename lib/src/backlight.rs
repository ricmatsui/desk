#[cfg(feature = "pi")]
use rppal::gpio::{Gpio, OutputPin};

pub struct Backlight {
    #[cfg(feature = "pi")]
    pin: OutputPin,
}

pub fn init() -> Backlight {
    #[cfg(feature = "pi")]
    {
        let gpio = Gpio::new().unwrap();

        return Backlight {
            pin: gpio.get(13).unwrap().into_output_low(),
        }
    }

    #[cfg(not(feature = "pi"))]
    Backlight {}
}
