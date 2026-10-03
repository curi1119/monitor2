// Shared layout keeps default and flat themes at identical sizes and positions.
use super::super::*;

pub(super) fn draw(painter: &mut Painter, app: &App) {
    painter.rect(0, app.scroll, WIDTH, app.height, BG);
    painter.panel(0);
    let cpus = displayed_cpus(app);
    let rows = core_rows(cpus.len());
    let usage_width = painter.percentage_width(3).max(28);
    let usage_x = WIDTH - 2 - usage_width;
    let usage_bar_width = usage_x - 38 - 2;
    let core_percent_width = painter.percentage_width(2).max(21);
    let logo = if app.snapshot.cpu_name.to_ascii_lowercase().contains("intel") {
        app.logos.0[0]
    } else {
        app.logos.0[1]
    };
    painter.icon(4, 4, 28, logo, "CPU");
    painter.text(40, 3, 94, 16, "CPU Usage", 0, TEXT);
    painter.text(
        usage_x,
        18,
        usage_width,
        14,
        &percentage(app.snapshot.cpu_total),
        3,
        TEXT,
    );
    painter.bar(
        38,
        24,
        usage_bar_width,
        6,
        app.snapshot.cpu_total,
        CORE_SHADES[0],
    );
    let cpu_name = app.snapshot.cpu_name.trim_end_matches(" Processor");
    let cpu_name = cpu_name
        .rsplit_once(' ')
        .filter(|(_, suffix)| suffix.ends_with("-Core"))
        .map_or(cpu_name, |(model, _)| model);
    painter.text(4, 36, WIDTH - 8, 12, cpu_name, 2, CPU_NAME);
    painter.memory(
        48,
        "RAM",
        app.snapshot.ram.map(|m| (m.used(), m.available, m.total)),
        MEMORY_SHADES,
    );
    painter.rect(6, 90, WIDTH - 12, 1, TRACK);
    for (i, cpu) in cpus.iter().enumerate() {
        let column = if cpus.len() <= 8 {
            0
        } else {
            i / rows as usize
        };
        let row = if cpus.len() <= 8 {
            i
        } else {
            i % rows as usize
        };
        let x = 4 + column as i32 * 68;
        let y = CORE_TOP + row as i32 * CORE_ROW;
        let width = if cpus.len() <= 8 { WIDTH - 8 } else { 64 };
        let number_width = if app.preferences.show_core_numbers {
            16
        } else {
            0
        };
        if app.preferences.show_core_numbers {
            painter.text(x, y, 16, CORE_ROW, &format!("{:02}", cpu.index), 2, MUTED);
        }
        let percent_width = if app.preferences.show_core_percent {
            core_percent_width
        } else {
            0
        };
        let bar_width = width - number_width - percent_width - 1;
        painter.bar(
            x + number_width,
            y + 2,
            bar_width,
            6,
            cpu.usage,
            CORE_SHADES[if painter.theme == Theme::Flat {
                0
            } else {
                i % CORE_SHADES.len()
            }],
        );
        if app.preferences.show_core_percent {
            painter.text(
                x + width - percent_width,
                y,
                percent_width,
                CORE_ROW,
                &percentage(cpu.usage),
                2,
                TEXT,
            );
        }
    }
    let gpu_start = cpu_panel_height(rows) + PANEL_GAP;
    for (i, gpu) in app.snapshot.gpus.iter().enumerate() {
        let y = gpu_start + i as i32 * (GPU_PANEL_HEIGHT + PANEL_GAP);
        painter.panel(y);
        painter.icon(4, y + 4, 28, app.logos.0[2], "GPU");
        painter.text(40, y + 3, 74, 16, "GPU Usage", 0, TEXT);
        painter.text(
            112,
            y + 3,
            24,
            14,
            &gpu.temperature.map_or("N/A".into(), |t| format!("{t}°")),
            3,
            TEMPERATURE,
        );
        painter.text(
            usage_x,
            y + 19,
            usage_width,
            14,
            &percentage(gpu.usage),
            3,
            TEXT,
        );
        painter.bar(38, y + 24, usage_bar_width, 6, gpu.usage, GPU_SHADES);
        painter.text(
            4,
            y + 36,
            WIDTH - 8,
            12,
            gpu.name.strip_prefix("NVIDIA ").unwrap_or(&gpu.name),
            2,
            GPU_NAME,
        );
        let memory = gpu
            .used
            .zip(gpu.total)
            .map(|(used, total)| (used, total.saturating_sub(used), total));
        painter.memory(y + 48, "VRAM", memory, MEMORY_SHADES);
    }
    if app.snapshot.gpus.is_empty() {
        painter.panel(gpu_start);
        painter.icon(4, gpu_start + 4, 28, app.logos.0[2], "GPU");
        painter.text(40, gpu_start + 3, 94, 16, "GPU Usage", 0, TEXT);
        painter.text(40, gpu_start + 19, 94, 14, "N/A", 1, MUTED);
    }
    if app.content_height > app.height {
        let track = app.height - 12;
        let thumb = (track * app.height / app.content_height).max(16);
        let y = 6 + app.scroll * (track - thumb) / (app.content_height - app.height);
        painter.rect(WIDTH - 5, app.scroll + y, 2, thumb, MUTED);
    }
    painter.frame(0, cpu_panel_height(rows));
    for i in 0..app.snapshot.gpus.len().max(1) {
        painter.frame(
            gpu_start + i as i32 * (GPU_PANEL_HEIGHT + PANEL_GAP),
            GPU_PANEL_HEIGHT,
        );
    }
}
