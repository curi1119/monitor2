use crate::hardware::{Snapshot, wide};
use std::{
    mem::size_of,
    ptr::{null, null_mut},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use windows_sys::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::LibraryLoader::GetModuleHandleW,
        UI::{HiDpi::*, Shell::*, WindowsAndMessaging::*},
    },
    core::w,
};

const TIMER: usize = 1;
const TRAY_MESSAGE: u32 = WM_APP + 1;
const WIDTH: i32 = 280;
const BG: u32 = 0x00211A16;
const TEXT: u32 = 0x00F3EDE5;
const MUTED: u32 = 0x00AD9B8A;
const CPU_COLOR: u32 = 0x00EAC879;
const RAM_COLOR: u32 = 0x00E1AFA7;
const GPU_COLOR: u32 = 0x00AEDC9D;
const TRACK: u32 = 0x003E3228;

pub fn show_error(error: &str) {
    let message = wide(error);
    // SAFETY: both strings are terminated and live for the synchronous call.
    unsafe {
        MessageBoxW(
            null_mut(),
            message.as_ptr(),
            w!("monitor2"),
            MB_OK | MB_ICONERROR,
        );
    }
}

struct GdiObjects {
    fonts: [HFONT; 3],
    dc: HDC,
    bitmap: HBITMAP,
    old_bitmap: HGDIOBJ,
    width: i32,
    height: i32,
}
impl GdiObjects {
    fn new(dpi: u32) -> Result<Self, String> {
        let mut objects = Self {
            fonts: [null_mut(); 3],
            dc: null_mut(),
            bitmap: null_mut(),
            old_bitmap: null_mut(),
            width: 0,
            height: 0,
        };
        objects.fonts(dpi)?;
        Ok(objects)
    }
    fn fonts(&mut self, dpi: u32) -> Result<(), String> {
        for font in &mut self.fonts {
            if !font.is_null() {
                unsafe {
                    DeleteObject(*font);
                }
                *font = null_mut();
            }
        }
        for (i, (size, weight)) in [(18, 700), (12, 600), (10, 400)].into_iter().enumerate() {
            // SAFETY: create owned GDI objects; sizes are scaled to the window DPI.
            self.fonts[i] = unsafe {
                CreateFontW(
                    -scale(size, dpi),
                    0,
                    0,
                    0,
                    weight,
                    0,
                    0,
                    0,
                    DEFAULT_CHARSET as u32,
                    0,
                    0,
                    CLEARTYPE_QUALITY as u32,
                    0,
                    w!("Segoe UI"),
                )
            };
            if self.fonts[i].is_null() {
                return Err("Cannot create UI font".into());
            }
        }
        Ok(())
    }
    fn buffer(&mut self, screen: HDC, width: i32, height: i32) -> bool {
        if self.width == width && self.height == height && !self.dc.is_null() {
            return true;
        }
        self.clear_buffer();
        // SAFETY: screen is the live BeginPaint DC. Bitmap is selected only into our private DC.
        unsafe {
            self.dc = CreateCompatibleDC(screen);
            self.bitmap = CreateCompatibleBitmap(screen, width, height);
            if self.dc.is_null() || self.bitmap.is_null() {
                self.clear_buffer();
                return false;
            }
            self.old_bitmap = SelectObject(self.dc, self.bitmap);
        }
        self.width = width;
        self.height = height;
        true
    }
    fn clear_buffer(&mut self) {
        // SAFETY: restore the borrowed bitmap before deleting our owned bitmap and DC.
        unsafe {
            if !self.dc.is_null() {
                if !self.old_bitmap.is_null() {
                    SelectObject(self.dc, self.old_bitmap);
                }
                DeleteDC(self.dc);
            }
            if !self.bitmap.is_null() {
                DeleteObject(self.bitmap);
            }
        }
        self.dc = null_mut();
        self.bitmap = null_mut();
        self.old_bitmap = null_mut();
    }
}
impl Drop for GdiObjects {
    fn drop(&mut self) {
        self.clear_buffer();
        for font in self.fonts {
            if !font.is_null() {
                unsafe {
                    DeleteObject(font);
                }
            }
        }
    }
}

struct App {
    latest: Arc<Mutex<Snapshot>>,
    snapshot: Snapshot,
    gdi: GdiObjects,
    dpi: u32,
    scroll: i32,
    content_height: i32,
    height: i32,
    topmost: bool,
    tray: NOTIFYICONDATAW,
    taskbar_message: u32,
    smoke_deadline: Option<Instant>,
}

pub fn run(latest: Arc<Mutex<Snapshot>>, smoke_test: bool, preview: bool) -> Result<(), String> {
    // SAFETY: process DPI mode is established before creating any window.
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
    let dpi = unsafe { GetDpiForSystem() }.max(96);
    let mut app = Box::new(App {
        latest,
        snapshot: Snapshot::default(),
        gdi: GdiObjects::new(dpi)?,
        dpi,
        scroll: 0,
        content_height: 400,
        height: 400,
        topmost: true,
        tray: NOTIFYICONDATAW::default(),
        taskbar_message: unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) },
        smoke_deadline: smoke_test.then(|| Instant::now() + Duration::from_secs(6)),
    });
    let instance = unsafe { GetModuleHandleW(null()) };
    let class = WNDCLASSW {
        lpfnWndProc: Some(window_proc),
        hInstance: instance,
        lpszClassName: w!("Monitor2Window"),
        hCursor: unsafe { LoadCursorW(null_mut(), IDC_ARROW) },
        ..Default::default()
    };
    // SAFETY: registered class and Box<App> remain live until the message loop ends.
    if unsafe { RegisterClassW(&class) } == 0 {
        return Err(last_error("RegisterClass"));
    }
    let hwnd = unsafe {
        CreateWindowExW(
            (if preview {
                WS_EX_APPWINDOW
            } else {
                WS_EX_TOOLWINDOW
            }) | WS_EX_TOPMOST,
            class.lpszClassName,
            w!("monitor2"),
            WS_POPUP,
            40,
            80,
            scale(WIDTH, dpi),
            scale(400, dpi),
            null_mut(),
            null_mut(),
            instance,
            (&mut *app as *mut App).cast(),
        )
    };
    if hwnd.is_null() {
        unsafe {
            UnregisterClassW(class.lpszClassName, instance);
        }
        return Err(last_error("CreateWindow"));
    }
    app.dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
    if let Err(error) = app.gdi.fonts(app.dpi) {
        unsafe {
            DestroyWindow(hwnd);
            UnregisterClassW(class.lpszClassName, instance);
        }
        return Err(error);
    }
    app.tray.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    app.tray.hWnd = hwnd;
    app.tray.uID = 1;
    app.tray.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    app.tray.uCallbackMessage = TRAY_MESSAGE;
    app.tray.hIcon = unsafe { LoadIconW(null_mut(), IDI_APPLICATION) };
    let tooltip = wide("monitor2 - CPU / RAM / NVIDIA GPU");
    app.tray.szTip[..tooltip.len()].copy_from_slice(&tooltip);
    unsafe {
        Shell_NotifyIconW(NIM_ADD, &app.tray);
        if SetTimer(hwnd, TIMER, 1000, None) == 0 {
            DestroyWindow(hwnd);
            UnregisterClassW(class.lpszClassName, instance);
            return Err(last_error("SetTimer"));
        }
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    }
    unsafe {
        refresh(hwnd, &mut *app);
    }
    let mut message = MSG::default();
    let result = loop {
        // SAFETY: standard UI-thread message loop; app lives throughout dispatch.
        let status = unsafe { GetMessageW(&mut message, null_mut(), 0, 0) };
        if status == -1 {
            break Err(last_error("GetMessage"));
        }
        if status == 0 {
            break Ok(());
        }
        unsafe {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    };
    // WM_DESTROY handles tray cleanup. Ensure the error path also destroys a live window.
    unsafe {
        if IsWindow(hwnd) != 0 {
            DestroyWindow(hwnd);
        }
        UnregisterClassW(class.lpszClassName, instance);
    }
    result
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_NCCREATE {
        // SAFETY: Windows supplies CREATESTRUCTW; lpCreateParams is our live Box<App>.
        let create = unsafe { &*(lparam as *const CREATESTRUCTW) };
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
        }
    }
    let pointer = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut App;
    if pointer.is_null() {
        return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
    }
    // SAFETY: only this UI thread accesses App; callbacks do not retain references.
    // Avoid synchronous message-producing APIs while a mutable App reference is active.
    match message {
        WM_TIMER => {
            let stop = unsafe {
                (*pointer)
                    .smoke_deadline
                    .is_some_and(|d| Instant::now() >= d)
            };
            if stop {
                unsafe {
                    PostMessageW(hwnd, WM_CLOSE, 0, 0);
                }
            } else {
                unsafe {
                    refresh(hwnd, pointer);
                }
            }
            0
        }
        WM_PAINT => {
            unsafe {
                paint(hwnd, &mut *pointer);
            }
            0
        }
        WM_ERASEBKGND => 1,
        WM_LBUTTONDOWN => {
            let dpi = unsafe { (*pointer).dpi };
            let x = (lparam as u32 & 0xffff) as i32;
            let y = ((lparam as u32 >> 16) & 0xffff) as i32;
            if x > scale(WIDTH - 36, dpi) && y < scale(38, dpi) {
                unsafe {
                    PostMessageW(hwnd, WM_CLOSE, 0, 0);
                }
            } else {
                unsafe {
                    windows_sys::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture();
                    SendMessageW(hwnd, WM_NCLBUTTONDOWN, HTCAPTION as usize, 0);
                }
            }
            0
        }
        WM_RBUTTONUP => {
            unsafe {
                menu(hwnd, pointer);
            }
            0
        }
        TRAY_MESSAGE => {
            if matches!(lparam as u32, WM_RBUTTONUP | WM_LBUTTONUP) {
                unsafe {
                    menu(hwnd, pointer);
                }
            }
            0
        }
        WM_MOUSEWHEEL => {
            let delta = ((wparam >> 16) as u16) as i16 as i32;
            unsafe {
                let app = &mut *pointer;
                app.scroll = (app.scroll - delta / 120 * 48)
                    .clamp(0, (app.content_height - app.height).max(0));
                InvalidateRect(hwnd, null(), 0);
            }
            0
        }
        WM_KEYDOWN if wparam == 27 => {
            unsafe {
                PostMessageW(hwnd, WM_CLOSE, 0, 0);
            }
            0
        }
        WM_DPICHANGED => {
            let dpi = (wparam & 0xffff) as u32;
            let suggested = unsafe { *(lparam as *const RECT) };
            unsafe {
                (*pointer).dpi = dpi;
                if let Err(error) = (*pointer).gdi.fonts(dpi) {
                    eprintln!("{error}");
                    PostMessageW(hwnd, WM_CLOSE, 0, 0);
                }
                SetWindowPos(
                    hwnd,
                    null_mut(),
                    suggested.left,
                    suggested.top,
                    suggested.right - suggested.left,
                    suggested.bottom - suggested.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
            unsafe {
                refresh(hwnd, pointer);
            }
            0
        }
        WM_CLOSE => {
            unsafe {
                DestroyWindow(hwnd);
            }
            0
        }
        WM_DESTROY => {
            unsafe {
                KillTimer(hwnd, TIMER);
                Shell_NotifyIconW(NIM_DELETE, &(*pointer).tray);
                PostQuitMessage(0);
            }
            0
        }
        WM_NCDESTROY => unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            DefWindowProcW(hwnd, message, wparam, lparam)
        },
        _ => {
            if message == unsafe { (*pointer).taskbar_message } {
                unsafe {
                    Shell_NotifyIconW(NIM_ADD, &(*pointer).tray);
                }
                0
            } else {
                unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
            }
        }
    }
}

unsafe fn refresh(hwnd: HWND, pointer: *mut App) {
    let (resized, height, dpi) = {
        // SAFETY: caller passes the UI-thread App that lives through the message loop.
        // End this mutable borrow before SetWindowPos can dispatch nested messages.
        let app = unsafe { &mut *pointer };
        app.snapshot = app.latest.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let rows = app.snapshot.cpus.len().div_ceil(4) as i32;
        app.content_height = 284 + rows * 26 + app.snapshot.gpus.len().max(1) as i32 * 140;
        let mut info = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let found = unsafe {
            GetMonitorInfoW(MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST), &mut info)
        } != 0;
        let max_height = if found {
            ((info.rcWork.bottom - info.rcWork.top - 32) * 96 / app.dpi as i32).max(120)
        } else {
            800
        };
        let height = app.content_height.min(max_height);
        let resized = height != app.height;
        app.height = height;
        app.scroll = app.scroll.clamp(0, (app.content_height - height).max(0));
        (resized, height, app.dpi)
    };
    if resized {
        unsafe {
            SetWindowPos(
                hwnd,
                null_mut(),
                0,
                0,
                scale(WIDTH, dpi),
                scale(height, dpi),
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }
    unsafe {
        InvalidateRect(hwnd, null(), 0);
    }
}
unsafe fn menu(hwnd: HWND, pointer: *mut App) {
    // Do not hold an App reference while TrackPopupMenu dispatches nested window messages.
    let topmost = unsafe { (*pointer).topmost };
    let menu = unsafe { CreatePopupMenu() };
    if menu.is_null() {
        return;
    }
    unsafe {
        AppendMenuW(
            menu,
            MF_STRING | if topmost { MF_CHECKED } else { 0 },
            1,
            w!("Always on top"),
        );
        AppendMenuW(menu, MF_STRING, 2, w!("Exit"));
    }
    let mut point = POINT::default();
    unsafe {
        GetCursorPos(&mut point);
        SetForegroundWindow(hwnd);
    }
    let choice = unsafe {
        TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON,
            point.x,
            point.y,
            0,
            hwnd,
            null(),
        )
    };
    unsafe {
        DestroyMenu(menu);
        PostMessageW(hwnd, WM_NULL, 0, 0);
    }
    match choice {
        1 => unsafe {
            (*pointer).topmost = !topmost;
            SetWindowPos(
                hwnd,
                if topmost {
                    HWND_NOTOPMOST
                } else {
                    HWND_TOPMOST
                },
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        },
        2 => unsafe {
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
        },
        _ => {}
    }
}

fn paint(hwnd: HWND, app: &mut App) {
    let mut ps = PAINTSTRUCT::default();
    // SAFETY: every BeginPaint is paired with EndPaint, including buffer allocation failure.
    let screen = unsafe { BeginPaint(hwnd, &mut ps) };
    let width = scale(WIDTH, app.dpi);
    let height = scale(app.height, app.dpi);
    let buffered = app.gdi.buffer(screen, width, height);
    let dc = if buffered { app.gdi.dc } else { screen };
    unsafe {
        SetBkMode(dc, TRANSPARENT as i32);
    }
    let old_font = unsafe { SelectObject(dc, app.gdi.fonts[1]) };
    let mut painter = Painter {
        dc,
        dpi: app.dpi,
        offset: app.scroll,
        fonts: app.gdi.fonts,
    };
    painter.rect(0, app.scroll, WIDTH, app.height, BG);
    painter.text(16, 16, 216, 28, "MONITOR", 0, TEXT);
    painter.text(WIDTH - 32, 17, 20, 24, "×", 1, MUTED);
    painter.text(16, 50, WIDTH - 32, 20, &app.snapshot.cpu_name, 2, MUTED);
    painter.text(16, 80, 200, 22, "CPU", 1, CPU_COLOR);
    painter.value(80, app.snapshot.cpu_total, CPU_COLOR);
    painter.bar(16, 108, WIDTH - 32, 8, app.snapshot.cpu_total, CPU_COLOR);
    let rows = app.snapshot.cpus.len().div_ceil(4) as i32;
    for (i, cpu) in app.snapshot.cpus.iter().enumerate() {
        let x = 16 + (i % 4) as i32 * 63;
        let y = 128 + (i / 4) as i32 * 26;
        let label = if app.snapshot.cpus.iter().any(|c| c.group != 0) {
            format!("{}:{}", cpu.group, cpu.index)
        } else {
            format!("{:02}", cpu.index)
        };
        let value = cpu.usage.map_or("--".into(), |v| format!("{v:.0}%"));
        painter.text(x, y, 58, 16, &format!("{label} {value}"), 2, MUTED);
        painter.bar(x, y + 18, 56, 3, cpu.usage, CPU_COLOR);
    }
    let ram_y = 144 + rows * 26;
    painter.rect(16, ram_y - 10, WIDTH - 32, 1, TRACK);
    painter.text(16, ram_y, 180, 24, "RAM", 1, RAM_COLOR);
    painter.value(ram_y, app.snapshot.ram.map(|m| m.percent()), RAM_COLOR);
    let ram_text = app.snapshot.ram.map_or("N/A".into(), |m| {
        format!("{:.1} / {:.1} GiB used", gib(m.used()), gib(m.total))
    });
    painter.text(16, ram_y + 30, WIDTH - 32, 20, &ram_text, 1, TEXT);
    let available = app.snapshot.ram.map_or("Available: N/A".into(), |m| {
        format!("Available: {:.1} GiB", gib(m.available))
    });
    painter.text(16, ram_y + 53, WIDTH - 32, 18, &available, 2, MUTED);
    painter.bar(
        16,
        ram_y + 80,
        WIDTH - 32,
        8,
        app.snapshot.ram.map(|m| m.percent()),
        RAM_COLOR,
    );
    let gpu_start = ram_y + 114;
    for (i, gpu) in app.snapshot.gpus.iter().enumerate() {
        let y = gpu_start + i as i32 * 140;
        painter.rect(16, y - 10, WIDTH - 32, 1, TRACK);
        painter.text(16, y, 110, 22, &format!("GPU {i}"), 1, GPU_COLOR);
        let temp = gpu.temperature.map_or("N/A".into(), |t| format!("{t} °C"));
        painter.text(134, y, 70, 22, &temp, 1, MUTED);
        painter.value(y, gpu.usage, GPU_COLOR);
        painter.text(16, y + 27, WIDTH - 32, 18, &gpu.name, 2, MUTED);
        painter.bar(16, y + 54, WIDTH - 32, 8, gpu.usage, GPU_COLOR);
        let vram = match (gpu.used, gpu.total) {
            (Some(used), Some(total)) => format!("VRAM  {:.1} / {:.1} GiB", gib(used), gib(total)),
            _ => "VRAM  N/A".into(),
        };
        painter.text(16, y + 76, WIDTH - 32, 20, &vram, 1, TEXT);
        let percent = gpu
            .used
            .zip(gpu.total)
            .map(|(u, t)| u as f64 * 100.0 / t as f64);
        painter.bar(16, y + 105, WIDTH - 32, 6, percent, GPU_COLOR);
    }
    if app.snapshot.gpus.is_empty() {
        painter.text(16, gpu_start, WIDTH - 32, 24, "GPU: starting...", 1, MUTED);
    }
    let status = if app.snapshot.sampled_at.is_none() {
        "STARTING"
    } else if app
        .snapshot
        .sampled_at
        .is_some_and(|t| t.elapsed() > Duration::from_secs(3))
    {
        "STALE"
    } else if app.snapshot.cpu_error.is_some()
        || app.snapshot.ram.is_none()
        || app.snapshot.gpus.iter().any(|g| g.error.is_some())
    {
        "PARTIAL · N/A"
    } else {
        "LIVE · 1 SEC"
    };
    painter.text(
        16,
        app.content_height - 25,
        WIDTH - 32,
        18,
        &format!("{status}   ·   drag / right-click"),
        2,
        MUTED,
    );
    if app.content_height > app.height {
        let track = app.height - 12;
        let thumb = (track * app.height / app.content_height).max(16);
        let y = 6 + app.scroll * (track - thumb) / (app.content_height - app.height);
        painter.rect(WIDTH - 5, app.scroll + y, 2, thumb, MUTED);
    }
    unsafe {
        SelectObject(dc, old_font);
        if buffered {
            BitBlt(screen, 0, 0, width, height, dc, 0, 0, SRCCOPY);
        }
        EndPaint(hwnd, &ps);
    }
}
struct Painter {
    dc: HDC,
    dpi: u32,
    offset: i32,
    fonts: [HFONT; 3],
}
impl Painter {
    fn rect(&mut self, x: i32, y: i32, width: i32, height: i32, color: u32) {
        let rect = RECT {
            left: scale(x, self.dpi),
            top: scale(y - self.offset, self.dpi),
            right: scale(x + width, self.dpi),
            bottom: scale(y + height - self.offset, self.dpi),
        };
        // SAFETY: stock brush is borrowed; changing its color allocates no GDI objects.
        unsafe {
            SetDCBrushColor(self.dc, color);
            FillRect(self.dc, &rect, GetStockObject(DC_BRUSH).cast());
        }
    }
    // Coordinate-based GDI helper: keep bounds, font and color explicit at call sites.
    #[allow(clippy::too_many_arguments)]
    fn text(
        &mut self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        text: &str,
        font: usize,
        color: u32,
    ) {
        let text = wide(text);
        let mut rect = RECT {
            left: scale(x, self.dpi),
            top: scale(y - self.offset, self.dpi),
            right: scale(x + width, self.dpi),
            bottom: scale(y + height - self.offset, self.dpi),
        };
        unsafe {
            SelectObject(self.dc, self.fonts[font]);
            SetTextColor(self.dc, color);
            DrawTextW(
                self.dc,
                text.as_ptr(),
                -1,
                &mut rect,
                DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX,
            );
        }
    }
    fn value(&mut self, y: i32, value: Option<f64>, color: u32) {
        let value = value.map_or("N/A".into(), |v| format!("{v:.0}%"));
        self.text(WIDTH - 68, y, 52, 24, &value, 1, color);
    }
    fn bar(&mut self, x: i32, y: i32, width: i32, height: i32, value: Option<f64>, color: u32) {
        self.rect(x, y, width, height, TRACK);
        if let Some(value) = value {
            self.rect(
                x,
                y,
                (width as f64 * value.clamp(0.0, 100.0) / 100.0).round() as i32,
                height,
                color,
            );
        }
    }
}
fn scale(value: i32, dpi: u32) -> i32 {
    (value * dpi as i32 + 48) / 96
}
fn gib(bytes: u64) -> f64 {
    bytes as f64 / 1_073_741_824.0
}
fn last_error(operation: &str) -> String {
    format!("{operation}: {}", std::io::Error::last_os_error())
}
