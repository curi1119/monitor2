use super::super::*;

pub(crate) const BG: u32 = rgb(0x021326);
pub(crate) const PANEL_TOP: u32 = rgb(0x071F47);
pub(crate) const TEXT: u32 = rgb(0xFFFFFF);
pub(crate) const BORDER: u32 = rgb(0x656566);
pub(crate) const MUTED: u32 = rgb(0xF0F8FF);
pub(crate) const CPU_NAME: u32 = rgb(0xFFD700);
pub(crate) const GPU_NAME: u32 = rgb(0x00FA9A);
pub(crate) const TEMPERATURE: u32 = rgb(0xFFC0CB);
pub(crate) const TRACK: u32 = rgb(0x5F5F5F);
pub(crate) const TRACK_SHADES: [u32; 2] = [rgb(0x5F5F5F), rgb(0x414141)];
pub(crate) const MEMORY_SHADES: [u32; 2] = [rgb(0x3399CC), rgb(0x1387B4)];
pub(crate) const GPU_SHADES: [u32; 2] = [rgb(0x009900), rgb(0x008700)];
// The legacy monitor's core colors, with bright upper and darker lower halves.
pub(crate) const CORE_SHADES: [[u32; 2]; 16] = [
    [rgb(0x66B3FF), rgb(0x1B71FE)],
    [rgb(0x00E3E3), rgb(0x00B6B6)],
    [rgb(0x6FE499), rgb(0x28BE45)],
    [rgb(0xFFE666), rgb(0xFEB322)],
    [rgb(0xFDA853), rgb(0xDC6C2C)],
    [rgb(0xF65051), rgb(0xBE2A2C)],
    [rgb(0xFC45D0), rgb(0xD9009D)],
    [rgb(0xCC66CC), rgb(0xA329A3)],
    [rgb(0x0066FF), rgb(0x0052CC)],
    MEMORY_SHADES,
    GPU_SHADES,
    [rgb(0x99CC00), rgb(0x989E00)],
    [rgb(0xCC6633), rgb(0xB2411B)],
    [rgb(0xCC0033), rgb(0x9E001C)],
    [rgb(0xCC00CC), rgb(0xB0009A)],
    [rgb(0xC35DD5), rgb(0xAD3AC7)],
];

pub(super) fn draw(painter: &mut Painter, app: &App) {
    super::detailed::draw(painter, app);
}

pub(crate) fn frame(painter: &mut Painter, y: i32, height: i32) {
    let diameter = scale(CORNER_DIAMETER, painter.dpi);
    // SAFETY: stock objects are borrowed. Restore both selections after drawing;
    // the hollow brush preserves the already painted contents. DC_PEN is one device pixel.
    unsafe {
        let old_pen = SelectObject(painter.dc, GetStockObject(DC_PEN));
        let old_brush = SelectObject(painter.dc, GetStockObject(HOLLOW_BRUSH));
        SetDCPenColor(painter.dc, BORDER);
        RoundRect(
            painter.dc,
            scale(1, painter.dpi),
            scale(y + 1 - painter.offset, painter.dpi),
            scale(WIDTH - 1, painter.dpi),
            scale(y + height - 1 - painter.offset, painter.dpi),
            diameter,
            diameter,
        );
        SelectObject(painter.dc, old_brush);
        SelectObject(painter.dc, old_pen);
    }
}

pub(crate) fn icon(painter: &mut Painter, x: i32, y: i32, size: i32, icon: HICON, _label: &str) {
    // SAFETY: owned icon sized for this DPI and live paint DC.
    unsafe {
        DrawIconEx(
            painter.dc,
            scale(x, painter.dpi),
            scale(y - painter.offset, painter.dpi),
            icon,
            scale(size, painter.dpi),
            scale(size, painter.dpi),
            0,
            null_mut(),
            DI_NORMAL,
        );
    }
    let edge = rgb(0x3F4D63);
    painter.rect(x - 1, y - 1, size + 2, 1, edge);
    painter.rect(x - 1, y + size, size + 2, 1, edge);
    painter.rect(x - 1, y, 1, size, edge);
    painter.rect(x + size, y, 1, size, edge);
}

pub(crate) fn panel(painter: &mut Painter, y: i32) {
    // A small number of solid bands approximates the legacy navy gradient.
    // No bitmap, brush, or per-pixel allocation is needed during painting.
    for band in 0..16 {
        let mut color = 0;
        for shift in [0, 8, 16] {
            let top = (PANEL_TOP >> shift) & 0xff;
            let bottom = (BG >> shift) & 0xff;
            color |= ((top * (15 - band) + bottom * band) / 15) << shift;
        }
        painter.rect(0, y + band as i32 * 4, WIDTH, 4, color);
    }
}

pub(crate) fn bar(
    painter: &mut Painter,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    value: Option<f64>,
    shades: [u32; 2],
) {
    let upper = height / 2;
    painter.rect(x, y, width, upper, TRACK_SHADES[0]);
    painter.rect(x, y + upper, width, height - upper, TRACK_SHADES[1]);
    if let Some(value) = value.filter(|value| value.is_finite()) {
        let filled = (width as f64 * value.clamp(0.0, 100.0) / 100.0).round() as i32;
        if filled > 0 {
            painter.rect(x, y, filled, upper, shades[0]);
            painter.rect(x, y + upper, filled, height - upper, shades[1]);
        }
    }
}
