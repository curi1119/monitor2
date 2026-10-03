// Prototype palette, adapted to the current shared layout without decoration.
use super::super::*;

pub(super) fn draw(painter: &mut Painter, app: &App) {
    super::detailed::draw(painter, app);
}

pub(super) fn color(color: u32) -> u32 {
    match color {
        BG | PANEL_TOP => rgb(0x161A21),
        TEXT => rgb(0xE5EDF3),
        MUTED => rgb(0x8A9BAD),
        CPU_NAME => rgb(0x79C8EA),
        GPU_NAME => rgb(0x9DDCAE),
        TEMPERATURE => rgb(0xE5EDF3),
        TRACK | 0x00414141 => rgb(0x28323E),
        _ if MEMORY_SHADES.contains(&color) => rgb(0xA7AFE1),
        _ if GPU_SHADES.contains(&color) => rgb(0x9DDCAE),
        _ if CORE_SHADES.iter().any(|pair| pair.contains(&color)) => rgb(0x79C8EA),
        _ => color,
    }
}

pub(super) fn icon(painter: &mut Painter, x: i32, y: i32, size: i32, label: &str) {
    painter.text(x, y, size, size, label, 2, MUTED);
}

#[allow(clippy::too_many_arguments)]
pub(super) fn bar(
    painter: &mut Painter,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    value: Option<f64>,
    color: u32,
) {
    painter.rect(x, y, width, height, TRACK);
    if let Some(value) = value.filter(|v| v.is_finite()) {
        let filled = (width as f64 * value.clamp(0.0, 100.0) / 100.0).round() as i32;
        if filled > 0 {
            painter.rect(x, y, filled, height, color);
        }
    }
}
