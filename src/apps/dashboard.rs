use raylib::prelude::*;

const SCREEN_SIZE: i32 = 240;
const HEADER_HEIGHT: i32 = 36;
const ROW_TOP: i32 = 44;
const ROW_HEIGHT: i32 = 30;
const MAX_ROWS: usize = 5;
const STATUS_BAR_WIDTH: i32 = 6;
const PADDING: i32 = 8;
const TEXT_SIZE: i32 = 20;
const FOOTER_TEXT_SIZE: i32 = 10;
const OFFLINE_ROW_ALPHA: f32 = 0.4;

const ALERT_COLOR: Color = Color::new(230, 41, 55, 255);
const WARN_COLOR: Color = Color::new(255, 161, 0, 255);
const NO_DATA_COLOR: Color = Color::new(130, 130, 130, 255);
const OK_COLOR: Color = Color::new(0, 168, 84, 255);
const OFFLINE_COLOR: Color = Color::new(120, 60, 200, 255);
const TEXT_COLOR: Color = Color::new(235, 235, 235, 255);
const DIM_TEXT_COLOR: Color = Color::new(140, 140, 140, 255);

#[derive(Debug, Clone, Default)]
pub struct Dashboard {
    pub monitors: Vec<Monitor>,
    /// Time of the last successful poll, if any
    pub polled_at: Option<chrono::DateTime<chrono::Local>>,
    /// Reason Datadog cannot be reached, monitors are from the last successful poll
    pub offline: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Monitor {
    pub name: String,
    pub status: Status,
    pub triggered_at: Option<chrono::DateTime<chrono::Local>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Status {
    Alert,
    Warn,
    NoData,
}

impl Dashboard {
    /// No failing monitors and Datadog is reachable
    pub fn is_clear(&self) -> bool {
        self.monitors.is_empty() && self.offline.is_none()
    }
}

/// Draws the dashboard, or a blank frame when there is nothing to show
pub fn draw(rl: &mut RaylibHandle, thread: &RaylibThread, dashboard: Option<&Dashboard>) {
    let mut d = rl.begin_drawing(thread);

    draw_dashboard(&mut d, dashboard);
}

pub fn draw_dashboard(d: &mut impl RaylibDraw, dashboard: Option<&Dashboard>) {
    d.clear_background(Color::BLACK);

    let Some(dashboard) = dashboard else {
        return;
    };

    draw_header(d, dashboard);

    let alpha = if dashboard.offline.is_some() {
        OFFLINE_ROW_ALPHA
    } else {
        1.0
    };

    let visible_rows = if dashboard.monitors.len() > MAX_ROWS {
        MAX_ROWS - 1
    } else {
        dashboard.monitors.len()
    };

    for (index, monitor) in dashboard.monitors.iter().take(visible_rows).enumerate() {
        draw_monitor(
            d,
            monitor,
            ROW_TOP + index as i32 * ROW_HEIGHT,
            dashboard.polled_at,
            alpha,
        );
    }

    if visible_rows < dashboard.monitors.len() {
        draw_centered_text(
            d,
            &format!("+{} more", dashboard.monitors.len() - visible_rows),
            ROW_TOP + visible_rows as i32 * ROW_HEIGHT + (ROW_HEIGHT - TEXT_SIZE) / 2,
            DIM_TEXT_COLOR.fade(alpha),
        );
    }

    if dashboard.offline.is_some() && dashboard.monitors.is_empty() {
        draw_centered_text(d, "Can't reach", 96, TEXT_COLOR);
        draw_centered_text(d, "Datadog", 96 + ROW_HEIGHT, TEXT_COLOR);
    }

    draw_footer(d, dashboard);
}

fn draw_header(d: &mut impl RaylibDraw, dashboard: &Dashboard) {
    let worst = dashboard.monitors.iter().map(|monitor| monitor.status).min();

    let (color, text) = match (&dashboard.offline, worst) {
        (Some(_), _) => (OFFLINE_COLOR, "OFFLINE".to_string()),
        (None, None) => (OK_COLOR, "ALL CLEAR".to_string()),
        (None, Some(worst)) => {
            // Only the worst status fits, the row bars show the rest
            let worst_count = dashboard
                .monitors
                .iter()
                .filter(|monitor| monitor.status == worst)
                .count();

            (worst.color(), format!("{} {}", worst_count, worst.label()))
        }
    };

    let text_color = if color == WARN_COLOR {
        Color::BLACK
    } else {
        Color::WHITE
    };

    d.draw_rectangle(0, 0, SCREEN_SIZE, HEADER_HEIGHT, color);
    d.draw_text(
        &text,
        PADDING,
        (HEADER_HEIGHT - TEXT_SIZE) / 2,
        TEXT_SIZE,
        text_color,
    );
}

fn draw_monitor(
    d: &mut impl RaylibDraw,
    monitor: &Monitor,
    y: i32,
    polled_at: Option<chrono::DateTime<chrono::Local>>,
    alpha: f32,
) {
    d.draw_rectangle(
        0,
        y + 3,
        STATUS_BAR_WIDTH,
        ROW_HEIGHT - 6,
        monitor.status.color().fade(alpha),
    );

    let text_y = y + (ROW_HEIGHT - TEXT_SIZE) / 2;

    let age = monitor
        .triggered_at
        .zip(polled_at)
        .map(|(triggered_at, polled_at)| format_age(polled_at - triggered_at))
        .unwrap_or_default();
    let age_width = measure_text(&age, TEXT_SIZE);
    d.draw_text(
        &age,
        SCREEN_SIZE - PADDING - age_width,
        text_y,
        TEXT_SIZE,
        DIM_TEXT_COLOR.fade(alpha),
    );

    let name_x = STATUS_BAR_WIDTH + PADDING;
    let name_width = SCREEN_SIZE - name_x - PADDING * 2 - age_width;
    d.draw_text(
        &truncate(&monitor.name, name_width),
        name_x,
        text_y,
        TEXT_SIZE,
        TEXT_COLOR.fade(alpha),
    );
}

/// Draws the offline reason on the left and the last successful poll time on the right
fn draw_footer(d: &mut impl RaylibDraw, dashboard: &Dashboard) {
    let y = SCREEN_SIZE - PADDING - FOOTER_TEXT_SIZE;

    if let Some(reason) = &dashboard.offline {
        d.draw_text(reason, PADDING, y, FOOTER_TEXT_SIZE, DIM_TEXT_COLOR);
    }

    let polled_text = match (dashboard.polled_at, &dashboard.offline) {
        (Some(polled_at), None) => polled_at.format("%H:%M").to_string(),
        (Some(polled_at), Some(_)) => polled_at.format("last ok %H:%M").to_string(),
        (None, _) => "never ok".to_string(),
    };
    let width = measure_text(&polled_text, FOOTER_TEXT_SIZE);
    d.draw_text(
        &polled_text,
        SCREEN_SIZE - PADDING - width,
        y,
        FOOTER_TEXT_SIZE,
        DIM_TEXT_COLOR,
    );
}

fn draw_centered_text(d: &mut impl RaylibDraw, text: &str, y: i32, color: Color) {
    let width = measure_text(text, TEXT_SIZE);
    d.draw_text(text, (SCREEN_SIZE - width) / 2, y, TEXT_SIZE, color);
}

impl Status {
    fn label(self) -> &'static str {
        match self {
            Status::Alert => "ALERT",
            Status::Warn => "WARN",
            Status::NoData => "NO DATA",
        }
    }

    fn color(self) -> Color {
        match self {
            Status::Alert => ALERT_COLOR,
            Status::Warn => WARN_COLOR,
            Status::NoData => NO_DATA_COLOR,
        }
    }
}

fn format_age(age: chrono::Duration) -> String {
    let minutes = age.num_minutes().max(0);

    if minutes < 60 {
        format!("{}m", minutes)
    } else if minutes < 60 * 24 {
        format!("{}h", minutes / 60)
    } else {
        format!("{}d", minutes / 60 / 24)
    }
}

/// Truncates text with an ellipsis to fit within width at the row text size
fn truncate(text: &str, width: i32) -> String {
    if measure_text(text, TEXT_SIZE) <= width {
        return text.to_string();
    }

    let mut truncated = text.to_string();

    while !truncated.is_empty() {
        truncated.pop();
        let candidate = format!("{}..", truncated.trim_end());
        if measure_text(&candidate, TEXT_SIZE) <= width {
            return candidate;
        }
    }

    String::new()
}
