#[cfg(feature = "pi")]
use rppal::pwm;
#[cfg(feature = "pi")]
use std::process::Command;

pub struct Backlight {
    #[cfg(feature = "pi")]
    pwm: pwm::Pwm,
}

impl Backlight {
    pub fn new() -> Self {
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

            return Self { pwm };
        }

        #[cfg(not(feature = "pi"))]
        Self {}
    }

    #[cfg(feature = "pi")]
    pub fn set_enabled(&mut self, enabled: bool) {
        Command::new("xset")
            .args(["dpms", "force", if enabled { "on" } else { "off" }])
            .status()
            .unwrap();
        set_backlight_pwm(&mut self.pwm, if enabled { 0.5 } else { 0.0 });
    }

    #[cfg(not(feature = "pi"))]
    pub fn set_enabled(&mut self, _enabled: bool) {}
}

#[cfg(feature = "pi")]
fn set_backlight_pwm(pwm: &mut pwm::Pwm, brightness: f64) {
    pwm.set_duty_cycle(brightness).unwrap();
}
