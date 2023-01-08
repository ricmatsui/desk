#[cfg(feature = "pi")]
use rppal::pwm;

pub struct Backlight {
    #[cfg(feature = "pi")]
    pwm: pwm::Pwm,
}

pub fn init() -> Backlight {
    #[cfg(feature = "pi")]
    {
        let mut pwm = pwm::Pwm::with_frequency(
            pwm::Channel::Pwm1,
            600_000.0,
            0.5,
            pwm::Polarity::Normal,
            true,
        )
        .unwrap();

        set_backlight_pwm(&mut pwm, 0.0);

        return Backlight { pwm };
    }

    #[cfg(not(feature = "pi"))]
    Backlight {}
}

#[cfg(feature = "pi")]
pub fn set_enabled(backlight: &mut Backlight, enabled: bool) {
    set_backlight_pwm(&mut backlight.pwm, if enabled { 0.5 } else { 0.0 });
}

#[cfg(feature = "pi")]
fn set_backlight_pwm(pwm: &mut pwm::Pwm, brightness: f64) {
    pwm.set_duty_cycle(brightness).unwrap();
}

#[cfg(not(feature = "pi"))]
pub fn set_enabled(_backlight: &mut Backlight, _enabled: bool) {}
