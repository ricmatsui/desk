use chrono::Datelike;
use chrono::Timelike;
use raylib::prelude::*;

pub struct SolarSystem {
    render_texture: RenderTexture2D,
}

impl SolarSystem {
    pub fn new(rl: &mut RaylibHandle, thread: &RaylibThread) -> Self {
        let render_texture = rl.load_render_texture(thread, 100, 100).unwrap();

        Self { render_texture }
    }

    pub fn draw_image(&mut self, d: &mut RaylibDrawHandle, thread: &RaylibThread) -> Image {
        let current_date = chrono::Utc::now();

        let mut months = vec![];

        for i in 0..12 {
            months.push((
                i,
                calculate_state(chrono::DateTime::from_naive_utc_and_offset(
                    current_date
                        .date_naive()
                        .with_day(1)
                        .unwrap()
                        .with_month(1 + i)
                        .unwrap()
                        .and_hms_opt(0, 0, 0)
                        .unwrap(),
                    chrono::Utc,
                )),
            ))
        }

        let mut moon_predictions = vec![];

        for i in 0..27 * 24 / 5 {
            moon_predictions.push(calculate_state(
                current_date + chrono::Duration::hours(i as i64 * 5),
            ));
        }

        let mut moon_ecliptics = vec![];
        let mut lunar_eclipses = vec![];
        let mut solar_eclipses = vec![];

        for window in moon_predictions.windows(2) {
            if let [previous, current] = window {
                if previous.moon_geopoint.lat.signum() != current.moon_geopoint.lat.signum() {
                    moon_ecliptics.push(previous);

                    if previous.moon_closest_full_phase_direction.abs() < 1.0 {
                        lunar_eclipses.push(previous);
                    }

                    if previous.moon_closest_new_phase_direction.abs() < 1.0 {
                        solar_eclipses.push(previous);
                    }
                }
            }
        }

        let mut earth_history = vec![];

        for i in 0..15 {
            earth_history.push(calculate_state(
                current_date - chrono::Duration::days(i as i64 * 4),
            ))
        }

        let mut moon_history = vec![];

        for i in 0..8 {
            moon_history.push(calculate_state(
                current_date - chrono::Duration::hours(i as i64 * 24),
            ))
        }

        let state = calculate_state(current_date);

        {
            let mut d = d.begin_texture_mode(thread, &mut self.render_texture);

            let mut d = d.begin_mode2D(Camera2D {
                rotation: 0.0,
                target: state.earth_position,
                offset: Vector2::new(50.0, 50.0),
                zoom: 2.0,
            });

            d.clear_background(Color::WHITE);

            for moon_ecliptic in moon_ecliptics {
                let point = state.earth_position + moon_ecliptic.moon_position
                    - moon_ecliptic.earth_position;

                if moon_ecliptic.date - state.date > chrono::Duration::days(20) {
                    continue;
                }

                d.draw_circle_v(point, 1.0, Color::color_from_hsv(0.0, 0.0, 0.38));
            }

            for lunar_eclipse in lunar_eclipses.iter() {
                let point = state.earth_position + lunar_eclipse.moon_position
                    - lunar_eclipse.earth_position;

                d.draw_circle_v(point, 2.8, Color::color_from_hsv(0.0, 0.0, 0.38));
            }

            for solar_eclipse in solar_eclipses.iter() {
                let point = state.earth_position + solar_eclipse.moon_position
                    - solar_eclipse.earth_position;

                d.draw_circle_v(point, 2.8, Color::color_from_hsv(0.0, 0.0, 0.38));
            }

            for (index, state) in months {
                let earth_position =
                    Vector2::new(state.earth_longitude.cos(), -state.earth_longitude.sin());

                if index % 3 == 0 {
                    d.draw_poly(
                        state.sun_position + earth_position * 49.0,
                        3,
                        7.0,
                        -state.earth_longitude.to_degrees() - 30.0,
                        Color::color_from_hsv(0.0, 0.0, 0.38),
                    );
                } else {
                    d.draw_line_ex(
                        state.sun_position + earth_position * 45.0,
                        state.sun_position + earth_position * 55.0,
                        2.0,
                        Color::color_from_hsv(0.0, 0.0, 0.38),
                    );
                }
            }

            for (i, window) in earth_history.windows(2).enumerate() {
                if let [previous, current] = window {
                    d.draw_line_ex(
                        previous.earth_position,
                        current.earth_position,
                        3.0,
                        Color::color_from_hsv(0.0, 0.0, 0.38)
                            .fade(1.0 - i as f32 / earth_history.len() as f32),
                    );
                }
            }

            d.draw_circle_v(state.moon_position, 5.0, Color::WHITE);

            for (i, window) in moon_history.windows(2).enumerate() {
                if let [previous, current] = window {
                    d.draw_line_ex(
                        state.earth_position + previous.moon_position - previous.earth_position,
                        state.earth_position + current.moon_position - current.earth_position,
                        3.0,
                        Color::color_from_hsv(0.0, 0.0, 0.38)
                            .fade(1.0 - i as f32 / moon_history.len() as f32),
                    );
                }
            }

            d.draw_circle_v(state.sun_position, 10.0, Color::BLACK);

            d.draw_circle_v(state.earth_position, 4.5, Color::BLACK);

            d.draw_circle_v(state.moon_position, 2.8, Color::BLACK);

            for lunar_eclipse in lunar_eclipses.iter() {
                let point = state.earth_position + lunar_eclipse.moon_position
                    - lunar_eclipse.earth_position;

                d.draw_circle_v(point, 1.8, Color::WHITE);
            }

            for solar_eclipse in solar_eclipses {
                let point = state.earth_position + solar_eclipse.moon_position
                    - solar_eclipse.earth_position;

                d.draw_circle_v(point, 1.8, Color::WHITE);
                d.draw_circle_v(point, 1.2, Color::BLACK);
            }
        }

        let mut image = self.render_texture.get_texture_data().unwrap();
        image.flip_vertical();

        {
            let mut d = d.begin_texture_mode(thread, &mut self.render_texture);

            d.clear_background(Color::BLACK);
            d.draw_circle_v(Vector2::new(50.0, 50.0), 50.0, Color::WHITE);
        }

        let mut mask = self.render_texture.get_texture_data().unwrap();
        mask.flip_vertical();

        image.alpha_mask(&mask);

        {
            let mut d = d.begin_texture_mode(thread, &mut self.render_texture);

            d.clear_background(Color::BLANK);
            d.draw_ring(state.sun_position, 48.5, 50.0, 0.0, 360.0, 40, Color::BLACK);
        }

        let mut ring = self.render_texture.get_texture_data().unwrap();
        ring.flip_vertical();

        image.draw(
            &ring,
            Rectangle::new(0.0, 0.0, ring.width() as f32, ring.height() as f32),
            Rectangle::new(0.0, 0.0, ring.width() as f32, ring.height() as f32),
            Color::WHITE,
        );

        image
    }
}

struct SolarSystemState {
    sun_position: Vector2,
    earth_position: Vector2,
    earth_longitude: f32,
    moon_position: Vector2,
    moon_geopoint: astro::coords::EclPoint,
    moon_closest_new_phase_direction: f64,
    moon_closest_full_phase_direction: f64,
    date: chrono::DateTime<chrono::Utc>,
}

fn calculate_state(date: chrono::DateTime<chrono::Utc>) -> SolarSystemState {
    let gregorian_date = astro::time::Date {
        year: date.year() as i16,
        month: date.month() as u8,
        decimal_day: astro::time::decimal_day(&astro::time::DayOfMonth {
            day: date.day() as u8,
            hr: date.hour() as u8,
            min: date.minute() as u8,
            sec: 0.0,
            time_zone: 0.0,
        }),
        cal_type: astro::time::CalType::Gregorian,
    };

    let julian_day = astro::time::julian_day(&gregorian_date);

    let (earth_longitude, _, earth_radius) =
        astro::planet::heliocent_coords(&astro::planet::Planet::Earth, julian_day);
    let earth_scale = 28.0;

    let (moon_geopoint, moon_radius) = astro::lunar::geocent_ecl_pos(julian_day);
    let moon_scale = 2.8 / 100000.0;

    let sun_position = Vector2::new(50.0, 50.0);

    let earth_position = sun_position
        + Vector2::new(
            (earth_longitude.cos() * earth_radius) as f32,
            (-earth_longitude.sin() * earth_radius) as f32,
        ) * earth_scale;

    let moon_position = earth_position
        + Vector2::new(
            (moon_geopoint.long.cos() * moon_radius) as f32,
            (-moon_geopoint.long.sin() * moon_radius) as f32,
        ) * moon_scale;

    let closest_new = astro::lunar::time_of_phase(&gregorian_date, &astro::lunar::Phase::New);
    let closest_full = astro::lunar::time_of_phase(&gregorian_date, &astro::lunar::Phase::Full);

    SolarSystemState {
        sun_position,
        earth_position,
        earth_longitude: earth_longitude as f32,
        moon_position,
        moon_geopoint,
        moon_closest_new_phase_direction: closest_new - julian_day,
        moon_closest_full_phase_direction: closest_full - julian_day,
        date,
    }
}
