use super::super::*;

pub(in crate::ui) const TRANSPARENT_COLOR: u32 = rgb(0xFF00FF);
pub(in crate::ui) const WIDTH: i32 = 100;
const ROW_HEIGHT: i32 = 12;
const BAR_COLOR: u32 = rgb(0xA8D848);

pub(super) fn content_height(gpu_count: usize) -> i32 {
    (2 + 2 * gpu_count.max(1) as i32) * ROW_HEIGHT
}

fn memory_usage(used: u64, total: u64) -> Option<f64> {
    (total > 0).then_some(used as f64 * 100.0 / total as f64)
}

pub(super) fn draw(painter: &mut Painter, app: &App) {
    painter.rect(0, app.scroll, WIDTH, app.height, TRANSPARENT_COLOR);
    row(painter, 0, "CPU", app.snapshot.cpu_total);
    row(
        painter,
        1,
        "RAM",
        app.snapshot
            .ram
            .and_then(|m| memory_usage(m.used(), m.total)),
    );
    for i in 0..app.snapshot.gpus.len().max(1) {
        let gpu = app.snapshot.gpus.get(i);
        let gpu_label = if app.snapshot.gpus.len() > 1 {
            format!("G{}", i + 1)
        } else {
            "GPU".into()
        };
        let vram_label = if app.snapshot.gpus.len() > 1 {
            format!("V{}", i + 1)
        } else {
            "VRAM".into()
        };
        row(
            painter,
            2 + i as i32 * 2,
            &gpu_label,
            gpu.and_then(|g| g.usage),
        );
        row(
            painter,
            3 + i as i32 * 2,
            &vram_label,
            gpu.and_then(|g| g.used.zip(g.total))
                .and_then(|(used, total)| memory_usage(used, total)),
        );
    }
}

fn row(painter: &mut Painter, index: i32, label: &str, value: Option<f64>) {
    let y = index * ROW_HEIGHT;
    // Labels and percentages share the bar's area; only the row gap is transparent.
    painter.rect(0, y + 1, WIDTH, 10, rgb(0x182028));
    if let Some(value) = value.filter(|v| v.is_finite()) {
        let filled = (WIDTH as f64 * value.clamp(0.0, 100.0) / 100.0).round() as i32;
        if filled > 0 {
            painter.rect(0, y + 1, filled, 10, BAR_COLOR);
        }
    }
    let value = percentage(value);
    for (x, width, text) in [(3, 32, label), (WIDTH - 31, 28, value.as_str())] {
        // Dark outline keeps the white label legible over both fill and track.
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            painter.text(x + dx, y + 1 + dy, width, 10, text, 2, rgb(0x101010));
        }
        painter.text(x, y + 1, width, 10, text, 2, TEXT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unavailable_memory_is_not_zero_usage() {
        assert_eq!(memory_usage(0, 0), None);
        assert_eq!(memory_usage(0, 1024), Some(0.0));
        assert_eq!(memory_usage(512, 1024), Some(50.0));
    }
}
