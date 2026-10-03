use super::*;
use crate::{
    hardware::{LogicalCpu, MemoryReading},
    nvml::GpuReading,
    settings::{Settings, SharedSettings},
};

// Render real GDI output without touching the user's windows or saved settings.
#[test]
fn themes_render_usage_unavailable_values_and_transparent_background() {
    for dpi in [96, 144, 192] {
        for theme in Theme::ALL {
            for unavailable in [false, true] {
                let preferences = Settings {
                    theme,
                    show_core_percent: true,
                    show_core_numbers: true,
                    ..Settings::default()
                };
                let usage = if unavailable { None } else { Some(100.0) };
                let snapshot = Snapshot {
                    cpu_name: "AMD Ryzen 7 5700X 8-Core Processor".into(),
                    cpu_total: usage,
                    cpus: (0..16)
                        .map(|index| LogicalCpu {
                            group: 0,
                            index,
                            usage,
                        })
                        .collect(),
                    ram: (!unavailable).then_some(MemoryReading {
                        total: 32 << 30,
                        available: 8 << 30,
                    }),
                    gpus: vec![GpuReading {
                        name: "NVIDIA GeForce RTX 4070".into(),
                        usage,
                        temperature: (!unavailable).then_some(60),
                        used: (!unavailable).then_some(6 << 30),
                        total: (!unavailable).then_some(12 << 30),
                        error: None,
                    }],
                    ..Snapshot::default()
                };
                let height = themes::content_height(theme, 8, 1);
                let width = themes::width(theme);
                let width_px = scale(width, dpi);
                let height_px = scale(height, dpi);
                let mut gdi = GdiObjects::new(dpi).unwrap();
                gdi.fonts_for_theme(dpi, theme).unwrap();
                let mut bits = null_mut();
                let info = BITMAPINFO {
                    bmiHeader: BITMAPINFOHEADER {
                        biSize: size_of::<BITMAPINFOHEADER>() as u32,
                        biWidth: width_px,
                        biHeight: -height_px,
                        biPlanes: 1,
                        biBitCount: 32,
                        biCompression: BI_RGB,
                        ..Default::default()
                    },
                    ..Default::default()
                };
                // SAFETY: test owns the memory DC/DIB. Pixel storage lives until
                // gdi drops; the original selected bitmap/font are restored first.
                unsafe {
                    gdi.dc = CreateCompatibleDC(null_mut());
                    assert!(!gdi.dc.is_null());
                    gdi.bitmap =
                        CreateDIBSection(gdi.dc, &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
                    assert!(!gdi.bitmap.is_null());
                    gdi.old_bitmap = SelectObject(gdi.dc, gdi.bitmap);
                    SetBkMode(gdi.dc, TRANSPARENT as i32);
                }
                let app = App {
                    latest: Arc::new(Mutex::new(snapshot.clone())),
                    snapshot,
                    gdi,
                    dpi,
                    scroll: 0,
                    wheel_remainder: 0,
                    content_height: height,
                    window_shape: None,
                    width,
                    height,
                    topmost: false,
                    settings: Arc::new(SharedSettings::new(preferences.clone())),
                    preferences,
                    dialog: null_mut(),
                    logos: Logos::new(dpi).unwrap(),
                    tray: NOTIFYICONDATAW::default(),
                    taskbar_message: 0,
                    smoke_test: true,
                    mouse_hook: None,
                };
                let mut painter = Painter {
                    theme,
                    dc: app.gdi.dc,
                    dpi,
                    offset: 0,
                    fonts: app.gdi.fonts,
                };
                let old_font = unsafe { SelectObject(painter.dc, painter.fonts[1]) };
                themes::draw(&mut painter, &app);
                unsafe {
                    SelectObject(painter.dc, old_font);
                    GdiFlush();
                    let expected = if theme == Theme::Overlay {
                        themes::overlay::TRANSPARENT_COLOR
                    } else {
                        themes::color(theme, BG)
                    };
                    assert_eq!(
                        GetPixel(
                            painter.dc,
                            scale(70, dpi),
                            scale(if theme == Theme::Overlay { 0 } else { 85 }, dpi)
                        ),
                        expected
                    );
                }
                if std::env::var_os("MONITOR2_RENDER_PREVIEWS").is_some() {
                    // Export the DIB as BMP for visual QA; never part of production.
                    let mut data = vec![0u8; 54];
                    let pixel_size = (width_px * height_px * 4) as usize;
                    data[0..2].copy_from_slice(b"BM");
                    data[2..6].copy_from_slice(&((54 + pixel_size) as u32).to_le_bytes());
                    data[10..14].copy_from_slice(&54u32.to_le_bytes());
                    data[14..18].copy_from_slice(&40u32.to_le_bytes());
                    data[18..22].copy_from_slice(&width_px.to_le_bytes());
                    data[22..26].copy_from_slice(&(-height_px).to_le_bytes());
                    data[26..28].copy_from_slice(&1u16.to_le_bytes());
                    data[28..30].copy_from_slice(&32u16.to_le_bytes());
                    let region = PanelShape {
                        theme,
                        width: width_px,
                        height: height_px,
                        dpi,
                        scroll: 0,
                        cpu_height: cpu_panel_height(8),
                        gpu_count: 1,
                    }
                    .region()
                    .unwrap();
                    // Apply the window shape to previews as well as the paint output.
                    let pixels = unsafe {
                        std::slice::from_raw_parts_mut(bits.cast::<u32>(), pixel_size / 4)
                    };
                    for y in 0..height_px {
                        for x in 0..width_px {
                            if unsafe { PtInRegion(region, x, y) } == 0 {
                                pixels[(y * width_px + x) as usize] = 0x00FF00FF;
                            }
                        }
                    }
                    unsafe {
                        DeleteObject(region.cast());
                    }
                    data.extend_from_slice(unsafe {
                        std::slice::from_raw_parts(bits.cast::<u8>(), pixel_size)
                    });
                    std::fs::create_dir_all("target/theme-previews").unwrap();
                    std::fs::write(
                        format!(
                            "target/theme-previews/{}-{dpi}-{}.bmp",
                            theme.key(),
                            if unavailable { "unavailable" } else { "usage" }
                        ),
                        data,
                    )
                    .unwrap();
                }
            }
        }
    }
}

#[test]
fn native_window_can_switch_transparency_back_and_forth() {
    // SAFETY: private invisible test window on this thread; class/window are
    // released before the test finishes. No user settings or registry writes.
    unsafe {
        let instance = GetModuleHandleW(null());
        let class = WNDCLASSW {
            lpfnWndProc: Some(DefWindowProcW),
            hInstance: instance,
            lpszClassName: w!("Monitor2ThemeTest"),
            ..Default::default()
        };
        assert_ne!(RegisterClassW(&class), 0);
        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW,
            class.lpszClassName,
            w!("Theme test"),
            WS_POPUP,
            0,
            0,
            140,
            280,
            null_mut(),
            null_mut(),
            instance,
            null(),
        );
        assert!(!hwnd.is_null());
        for theme in [
            Theme::Default,
            Theme::Overlay,
            Theme::Flat,
            Theme::Overlay,
            Theme::Default,
        ] {
            apply_window_theme(hwnd, theme, false).unwrap();
            let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
            assert_eq!(style & WS_EX_LAYERED != 0, theme == Theme::Overlay);
            assert_eq!(style & WS_EX_NOACTIVATE != 0, theme == Theme::Overlay);
            assert_eq!(style & WS_EX_TOPMOST, 0);
            if theme == Theme::Overlay {
                let (mut key, mut alpha, mut flags) = (0, 0, 0);
                assert_ne!(
                    GetLayeredWindowAttributes(hwnd, &mut key, &mut alpha, &mut flags),
                    0
                );
                assert_eq!(key, themes::overlay::TRANSPARENT_COLOR);
                assert_eq!(flags, LWA_COLORKEY);
            }
        }
        for theme in Theme::ALL {
            for enabled in [true, false, true, false] {
                apply_window_theme(hwnd, theme, enabled).unwrap();
                let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
                assert_eq!(style & WS_EX_TRANSPARENT != 0, enabled);
                assert_eq!(
                    style & WS_EX_LAYERED != 0,
                    enabled || theme == Theme::Overlay
                );
                if enabled {
                    let hook = input::MouseHook::new(hwnd, true, theme).unwrap();
                    drop(hook);
                }
            }
        }
        DestroyWindow(hwnd);
        UnregisterClassW(class.lpszClassName, instance);
    }
}
