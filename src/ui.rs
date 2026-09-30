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
const WIDTH: i32 = 172;
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
        for (i, (size, weight)) in [(13, 600), (12, 500), (11, 400)].into_iter().enumerate() {
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

// Owned icons loaded at the exact pixel size used for drawing. LR_SHARED caches by
// resource name irrespective of requested size, so these handles must be destroyed.
struct Logos([HICON; 3]);
impl Logos {
    fn new(dpi: u32) -> Result<Self, String> {
        let mut icons = Self([null_mut(); 3]);
        let instance = unsafe { GetModuleHandleW(null()) };
        for (i, icon) in icons.0.iter_mut().enumerate() {
            *icon = unsafe {
                LoadImageW(
                    instance,
                    (i + 2) as _,
                    IMAGE_ICON,
                    scale(28, dpi),
                    scale(28, dpi),
                    0,
                )
            }
            .cast();
            if icon.is_null() {
                return Err(last_error("Load brand icon"));
            }
        }
        Ok(icons)
    }
}
impl Drop for Logos {
    fn drop(&mut self) {
        for icon in self.0 {
            if !icon.is_null() {
                unsafe {
                    DestroyIcon(icon);
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
    settings: Arc<crate::settings::SharedSettings>,
    preferences: crate::settings::Settings,
    dialog: HWND,
    logos: Logos,
    tray: NOTIFYICONDATAW,
    taskbar_message: u32,
    smoke_deadline: Option<Instant>,
}

pub fn run(
    latest: Arc<Mutex<Snapshot>>,
    settings: Arc<crate::settings::SharedSettings>,
    smoke_test: bool,
    preview: bool,
) -> Result<(), String> {
    // SAFETY: process DPI mode is established before creating any window.
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
    let dpi = unsafe { GetDpiForSystem() }.max(96);
    let preferences = settings.get();
    let instance = unsafe { GetModuleHandleW(null()) };
    let resource_icon = |id: usize| unsafe { LoadIconW(instance, id as _) };
    let mut app = Box::new(App {
        latest,
        snapshot: Snapshot::default(),
        gdi: GdiObjects::new(dpi)?,
        dpi,
        scroll: 0,
        content_height: 320,
        height: 320,
        topmost: preferences.topmost,
        settings,
        preferences,
        dialog: null_mut(),
        logos: Logos::new(dpi)?,
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
        hIcon: resource_icon(1),
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
            }) | if app.topmost { WS_EX_TOPMOST } else { 0 },
            class.lpszClassName,
            w!("monitor2"),
            WS_POPUP,
            40,
            80,
            scale(WIDTH, dpi),
            scale(320, dpi),
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
    app.tray.hIcon = resource_icon(1);
    let tooltip = wide("monitor2 - CPU / RAM / NVIDIA GPU");
    app.tray.szTip[..tooltip.len()].copy_from_slice(&tooltip);
    unsafe {
        Shell_NotifyIconW(NIM_ADD, &app.tray);
        if SetTimer(hwnd, TIMER, app.preferences.interval_ms, None) == 0 {
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
        if !app.dialog.is_null() && unsafe { IsDialogMessageW(app.dialog, &message) } != 0 {
            continue;
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
            unsafe {
                windows_sys::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture();
                SendMessageW(hwnd, WM_NCLBUTTONDOWN, HTCAPTION as usize, 0);
            }
            0
        }
        crate::settings_ui::CHANGED => {
            let preferences = unsafe { (*pointer).settings.get() };
            let topmost = preferences.topmost;
            unsafe {
                (*pointer).topmost = topmost;
                SetTimer(hwnd, TIMER, preferences.interval_ms, None);
                (*pointer).preferences = preferences;
                SetWindowPos(
                    hwnd,
                    if topmost {
                        HWND_TOPMOST
                    } else {
                        HWND_NOTOPMOST
                    },
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                );
                refresh(hwnd, pointer);
            }
            0
        }
        crate::settings_ui::CLOSED => {
            unsafe {
                (*pointer).dialog = null_mut();
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
                match Logos::new(dpi) {
                    Ok(logos) => (*pointer).logos = logos,
                    Err(error) => {
                        eprintln!("{error}");
                        PostMessageW(hwnd, WM_CLOSE, 0, 0);
                    }
                }
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
                let dialog = (*pointer).dialog;
                (*pointer).dialog = null_mut();
                if !dialog.is_null() {
                    DestroyWindow(dialog);
                }
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
        let stale = app.snapshot.sampled_at.is_some_and(|at| {
            at.elapsed() > Duration::from_millis((app.preferences.interval_ms as u64 * 3).max(3000))
        });
        if stale {
            app.snapshot.cpu_total = None;
            for cpu in app
                .snapshot
                .cpus
                .iter_mut()
                .chain(app.snapshot.physical_cpus.iter_mut())
            {
                cpu.usage = None;
            }
            app.snapshot.ram = None;
            for gpu in &mut app.snapshot.gpus {
                gpu.usage = None;
                gpu.temperature = None;
                gpu.used = None;
                gpu.total = None;
            }
        }
        let tip = if stale {
            "monitor2 - STALE".to_string()
        } else if let Some(error) = app
            .snapshot
            .cpu_error
            .as_ref()
            .or_else(|| app.snapshot.gpus.iter().find_map(|g| g.error.as_ref()))
        {
            format!("monitor2 - {error}")
        } else {
            "monitor2 - CPU / RAM / NVIDIA GPU".to_string()
        };
        let mut tooltip = [0u16; 128];
        for (slot, value) in tooltip.iter_mut().take(127).zip(tip.encode_utf16()) {
            *slot = value;
        }
        if app.tray.szTip != tooltip {
            app.tray.szTip = tooltip;
            unsafe {
                Shell_NotifyIconW(NIM_MODIFY, &app.tray);
            }
        }

        let cpus = displayed_cpus(app);
        let rows = core_rows(cpus.len());
        app.content_height = 112 + rows * 13 + app.snapshot.gpus.len().max(1) as i32 * 104;
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
    let menu = unsafe { CreatePopupMenu() };
    if menu.is_null() {
        return;
    }
    unsafe {
        AppendMenuW(menu, MF_STRING, 1, w!("設定"));
        AppendMenuW(menu, MF_STRING, 2, w!("終了"));
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
        1 => {
            let dialog = unsafe { (*pointer).dialog };
            if !dialog.is_null() {
                unsafe {
                    SetForegroundWindow(dialog);
                }
            } else {
                let shared = unsafe { Arc::clone(&(*pointer).settings) };
                match crate::settings_ui::open(hwnd, shared) {
                    Ok(dialog) => unsafe {
                        (*pointer).dialog = dialog;
                    },
                    Err(error) => show_error(&error),
                }
            }
        }
        2 => unsafe {
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
        },
        _ => {}
    }
}
fn displayed_cpus(app: &App) -> &[crate::hardware::LogicalCpu] {
    if app.preferences.physical_cores {
        &app.snapshot.physical_cpus
    } else {
        &app.snapshot.cpus
    }
}
fn core_rows(count: usize) -> i32 {
    if count <= 8 {
        count as i32
    } else {
        count.div_ceil(2) as i32
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
    let cpus = displayed_cpus(app);
    let rows = core_rows(cpus.len());
    let logo = if app.snapshot.cpu_name.to_ascii_lowercase().contains("intel") {
        app.logos.0[0]
    } else {
        app.logos.0[1]
    };
    painter.icon(4, 4, 28, logo);
    painter.text(38, 2, 98, 18, "CPU Usage", 0, TEXT);
    painter.text(
        124,
        16,
        44,
        18,
        &percentage(app.snapshot.cpu_total),
        1,
        TEXT,
    );
    painter.bar(38, 23, 78, 8, app.snapshot.cpu_total, CPU_COLOR);
    let cpu_name = app.snapshot.cpu_name.trim_end_matches(" Processor");
    let cpu_name = cpu_name
        .rsplit_once(' ')
        .filter(|(_, suffix)| suffix.ends_with("-Core"))
        .map_or(cpu_name, |(model, _)| model);
    painter.text(4, 33, WIDTH - 8, 16, cpu_name, 2, CPU_COLOR);
    painter.memory(
        49,
        "RAM",
        app.snapshot.ram.map(|m| (m.used(), m.available, m.total)),
        RAM_COLOR,
    );
    painter.rect(6, 97, WIDTH - 12, 1, TRACK);
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
        let x = 4 + column as i32 * 86;
        let y = 101 + row as i32 * 13;
        let width = if cpus.len() <= 8 { WIDTH - 8 } else { 78 };
        let number_width = if app.preferences.show_core_numbers {
            20
        } else {
            0
        };
        if app.preferences.show_core_numbers {
            painter.text(x, y, 20, 13, &format!("{:02}", cpu.index), 2, MUTED);
        }
        let percent_width = if app.preferences.show_core_percent {
            30
        } else {
            0
        };
        let bar_width = width - number_width - percent_width - 2;
        let colors = [0x004C9AFF, 0x006DDDB1, 0x00FFB873, 0x00D795D8];
        painter.bar(
            x + number_width,
            y + 3,
            bar_width,
            8,
            cpu.usage,
            colors[i % colors.len()],
        );
        if app.preferences.show_core_percent {
            painter.text(x + width - 30, y, 30, 13, &percentage(cpu.usage), 2, TEXT);
        }
    }
    let gpu_start = 108 + rows * 13;
    for (i, gpu) in app.snapshot.gpus.iter().enumerate() {
        let y = gpu_start + i as i32 * 104;
        painter.rect(6, y - 5, WIDTH - 12, 1, TRACK);
        painter.icon(4, y + 1, 28, app.logos.0[2]);
        painter.text(38, y, 98, 18, "GPU Usage", 0, TEXT);
        painter.text(
            130,
            y,
            38,
            16,
            &gpu.temperature.map_or("N/A".into(), |t| format!("{t}°")),
            2,
            GPU_COLOR,
        );
        painter.text(124, y + 16, 44, 18, &percentage(gpu.usage), 1, TEXT);
        painter.bar(38, y + 23, 78, 8, gpu.usage, GPU_COLOR);
        painter.text(
            4,
            y + 33,
            WIDTH - 8,
            16,
            gpu.name.strip_prefix("NVIDIA ").unwrap_or(&gpu.name),
            2,
            GPU_COLOR,
        );
        let memory = gpu
            .used
            .zip(gpu.total)
            .map(|(used, total)| (used, total.saturating_sub(used), total));
        painter.memory(y + 49, "VRAM", memory, GPU_COLOR);
    }
    if app.snapshot.gpus.is_empty() {
        painter.icon(4, gpu_start, 28, app.logos.0[2]);
        painter.text(38, gpu_start, 98, 18, "GPU Usage", 0, TEXT);
        painter.text(38, gpu_start + 18, 98, 18, "N/A", 1, MUTED);
    }
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
    fn icon(&mut self, x: i32, y: i32, size: i32, icon: HICON) {
        // SAFETY: owned icon sized for this DPI and live paint DC.
        unsafe {
            DrawIconEx(
                self.dc,
                scale(x, self.dpi),
                scale(y - self.offset, self.dpi),
                icon,
                scale(size, self.dpi),
                scale(size, self.dpi),
                0,
                null_mut(),
                DI_NORMAL,
            );
        }
    }
    fn memory(&mut self, y: i32, label: &str, memory: Option<(u64, u64, u64)>, color: u32) {
        for (x, title) in [(6, "Used"), (62, "Avail"), (118, "Total")] {
            self.text(x, y, 48, 14, title, 2, MUTED);
        }
        let values = memory.map(|m| [m.0, m.1, m.2]);
        for (i, x) in [6, 62, 118].into_iter().enumerate() {
            let text = values.map_or("N/A".into(), |v| format!("{:.1}G", gib(v[i])));
            self.text(x, y + 14, 48, 16, &text, 1, TEXT);
        }
        self.text(4, y + 32, 33, 16, label, 2, MUTED);
        let usage = memory.and_then(|(u, _, t)| (t > 0).then_some(u as f64 * 100.0 / t as f64));
        self.bar(40, y + 36, WIDTH - 46, 8, usage, color);
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

fn percentage(value: Option<f64>) -> String {
    value.map_or("N/A".into(), |v| format!("{v:.0}%"))
}
