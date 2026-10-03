pub(super) mod default;
mod detailed;
mod flat;
pub(super) mod overlay;

use super::*;

pub(super) fn draw(painter: &mut Painter, app: &App) {
    match app.preferences.theme {
        Theme::Default => default::draw(painter, app),
        Theme::Flat => flat::draw(painter, app),
        Theme::Overlay => overlay::draw(painter, app),
    }
}

pub(super) fn content_height(theme: Theme, rows: i32, gpu_count: usize) -> i32 {
    if theme == Theme::Overlay {
        overlay::content_height(gpu_count)
    } else {
        cpu_panel_height(rows) + gpu_count.max(1) as i32 * (PANEL_GAP + GPU_PANEL_HEIGHT)
    }
}
pub(super) fn width(theme: Theme) -> i32 {
    if theme == Theme::Overlay {
        overlay::WIDTH
    } else {
        WIDTH
    }
}

pub(super) fn color(theme: Theme, color: u32) -> u32 {
    if theme == Theme::Flat {
        flat::color(color)
    } else {
        color
    }
}

pub(super) fn frame(painter: &mut Painter, y: i32, height: i32) {
    if painter.theme == Theme::Default {
        default::frame(painter, y, height);
    }
}
pub(super) fn panel(painter: &mut Painter, y: i32) {
    if painter.theme == Theme::Default {
        default::panel(painter, y);
    }
}
pub(super) fn icon(painter: &mut Painter, x: i32, y: i32, size: i32, icon: HICON, label: &str) {
    if painter.theme == Theme::Default {
        default::icon(painter, x, y, size, icon, label);
    } else {
        flat::icon(painter, x, y, size, label);
    }
}
#[allow(clippy::too_many_arguments)]
pub(super) fn bar(
    painter: &mut Painter,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    value: Option<f64>,
    shades: [u32; 2],
) {
    if painter.theme == Theme::Default {
        default::bar(painter, x, y, width, height, value, shades);
    } else {
        flat::bar(painter, x, y, width, height, value, shades[0]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flat_keeps_default_layout_and_overlay_ignores_core_count() {
        for rows in [0, 4, 8, 32] {
            for gpus in [0, 1, 2, 4] {
                assert_eq!(
                    content_height(Theme::Default, rows, gpus),
                    content_height(Theme::Flat, rows, gpus)
                );
                assert_eq!(
                    content_height(Theme::Overlay, rows, gpus),
                    24 + 24 * gpus.max(1) as i32
                );
            }
        }
        assert_eq!(content_height(Theme::Default, 8, 1), 280);
        assert_eq!(content_height(Theme::Overlay, 8, 1), 48);
        assert_eq!(width(Theme::Overlay), 100);
        assert_eq!(width(Theme::Default), width(Theme::Flat));
    }
}
