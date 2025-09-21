use raylib::prelude::*;

pub fn weather_forecast(
    image: &mut Image,
    font_solid: &Font,
) -> Result<(), Box<dyn std::error::Error>> {
    let now = chrono::Local::now();

    let client = reqwest::blocking::Client::new();

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::CONTENT_TYPE,
        reqwest::header::HeaderValue::from_static("application/json"),
    );

    let response = client
        .get("https://api.open-meteo.com/v1/forecast")
        .query(&[
            ("latitude", std::env::var("LATITUDE").unwrap()),
            ("longitude", std::env::var("LONGITUDE").unwrap()),
        ])
        .query(&[
            ("hourly", "temperature_2m,precipitation_probability"),
            ("temperature_unit", "fahrenheit"),
            ("wind_speed_unit", "mph"),
            ("precipitation_unit", "inch"),
            ("timezone", "America/Los_Angeles"),
            ("forecast_days", "2"),
        ])
        .headers(headers)
        .send()?
        .error_for_status()?
        .json::<serde_json::Value>()?;

    let forecast_length = response["hourly"]["temperature_2m"]
        .as_array()
        .unwrap()
        .len();

    let start_of_day = now.with_time(chrono::NaiveTime::MIN).unwrap();
    let tomorrow = start_of_day + chrono::Duration::days(1);
    let mid_day = start_of_day + chrono::Duration::hours(12);
    let tomorrow_mid_day = start_of_day + chrono::Duration::days(1) + chrono::Duration::hours(12);

    let mut hot_times = vec![];
    let mut hot_start_index = None;

    let hot_start_threshold = 75.0;
    let hot_end_threshold = 72.0;

    let mut cold_times = vec![];
    let mut cold_start_index = None;

    let cold_start_threshold = 50.0;
    let cold_end_threshold = 55.0;

    for i in 0..forecast_length {
        let time = chrono::NaiveDateTime::parse_from_str(
            response["hourly"]["time"][i].as_str().unwrap(),
            "%Y-%m-%dT%H:%M",
        )
        .unwrap()
        .and_local_timezone(chrono::Local)
        .unwrap();

        if time < start_of_day {
            continue;
        }

        let current_temperature = response["hourly"]["temperature_2m"][i].as_f64().unwrap();

        if hot_start_index.is_none() && current_temperature > hot_start_threshold && time < tomorrow
        {
            hot_start_index = Some(i);
        }

        if hot_start_index.is_some()
            && (i == forecast_length - 1 || current_temperature < hot_end_threshold)
        {
            hot_times.push((
                chrono::NaiveDateTime::parse_from_str(
                    response["hourly"]["time"][hot_start_index.unwrap()]
                        .as_str()
                        .unwrap(),
                    "%Y-%m-%dT%H:%M",
                )
                .unwrap(),
                chrono::NaiveDateTime::parse_from_str(
                    response["hourly"]["time"][i].as_str().unwrap(),
                    "%Y-%m-%dT%H:%M",
                )
                .unwrap(),
            ));
            hot_start_index = None;
        }

        if cold_start_index.is_none()
            && current_temperature < cold_start_threshold
            && time > mid_day
            && time < tomorrow_mid_day
        {
            cold_start_index = Some(i);
        }

        if cold_start_index.is_some()
            && (i == forecast_length - 1 || current_temperature > cold_end_threshold)
        {
            cold_times.push((
                chrono::NaiveDateTime::parse_from_str(
                    response["hourly"]["time"][cold_start_index.unwrap()]
                        .as_str()
                        .unwrap(),
                    "%Y-%m-%dT%H:%M",
                )
                .unwrap(),
                chrono::NaiveDateTime::parse_from_str(
                    response["hourly"]["time"][i].as_str().unwrap(),
                    "%Y-%m-%dT%H:%M",
                )
                .unwrap(),
            ));
            cold_start_index = None;
        }
    }

    let hot_image = Image::load_image_from_mem(
        ".png",
        &HOT_IMAGE_DATA.to_vec(),
        HOT_IMAGE_DATA.len() as i32,
    )
    .unwrap();

    let cold_image = Image::load_image_from_mem(
        ".png",
        &COLD_IMAGE_DATA.to_vec(),
        COLD_IMAGE_DATA.len() as i32,
    )
    .unwrap();

    let mut y = 10;

    image.draw_rectangle(
        5,
        y - 5,
        100 + 10,
        25 * (hot_times.len() + cold_times.len()) as i32 + 10,
        Color::WHITE,
    );

    for (start, end) in hot_times {
        image.draw(
            &hot_image,
            Rectangle::new(0.0, 0.0, hot_image.width as f32, hot_image.height as f32),
            Rectangle::new(10.0, y as f32, 20.0, 20.0),
            Color::WHITE,
        );

        let start_string = start.format("%-I%P").to_string();
        let end_string = end.format("%-I%P").to_string();
        image.draw_text_ex(
            &font_solid,
            &format!(
                "{}-{}",
                start_string[..start_string.len() - 1].to_string(),
                end_string[..end_string.len() - 1].to_string()
            ),
            Vector2::new(10.0 + 20.0 + 5.0, (y - 5) as f32),
            30.0,
            0.0,
            Color::BLACK,
        );

        y += 25;
    }

    for (start, end) in cold_times {
        image.draw(
            &cold_image,
            Rectangle::new(0.0, 0.0, cold_image.width as f32, cold_image.height as f32),
            Rectangle::new(10.0, y as f32, 20.0, 20.0),
            Color::WHITE,
        );

        let start_string = start.format("%-I%P").to_string();
        let end_string = end.format("%-I%P").to_string();
        image.draw_text_ex(
            &font_solid,
            &format!(
                "{}-{}",
                start_string[..start_string.len() - 1].to_string(),
                end_string[..end_string.len() - 1].to_string()
            ),
            Vector2::new(10.0 + 20.0 + 5.0, (y - 5) as f32),
            20.0,
            0.0,
            Color::BLACK,
        );

        y += 25;
    }

    Ok(())
}

static HOT_IMAGE_DATA: &[u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/heat-wave.png"));

static COLD_IMAGE_DATA: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/thermometer-minus.png"
));
