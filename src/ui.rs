use crate::hardware::{Snapshot, wide};
use crate::settings::Theme;
#[cfg(test)]
mod render_tests;
mod themes;
use std::{
    mem::size_of,
    ptr::{null, null_mut},
    sync::{Arc, Mutex},
    time::Duration,
};
use themes::default::*;
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
const SMOKE_TIMER: usize = 2;
const TRAY_MESSAGE: u32 = WM_APP + 1;
const WIDTH: i32 = 140;
const CORE_TOP: i32 = 94;
const CORE_ROW: i32 = 10;
const GPU_PANEL_HEIGHT: i32 = 96;
const PANEL_GAP: i32 = 2;
const INITIAL_HEIGHT: i32 = 280;
const CORNER_DIAMETER: i32 = 6;
// RGB literals are converted to GDI's COLORREF (0x00BBGGRR).
const fn rgb(value: u32) -> u32 {
    ((value & 0xff) << 16) | (value & 0xff00) | ((value >> 16) & 0xff)
}
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
    fonts: [HFONT; 4],
    dc: HDC,
    bitmap: HBITMAP,
    old_bitmap: HGDIOBJ,
    width: i32,
    height: i32,
}
impl GdiObjects {
    fn new(dpi: u32) -> Result<Self, String> {
        let mut objects = Self {
            fonts: [null_mut(); 4],
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
        self.fonts_for_theme(dpi, Theme::Default)
    }
    fn fonts_for_theme(&mut self, dpi: u32, theme: Theme) -> Result<(), String> {
        for font in &mut self.fonts {
            if !font.is_null() {
                unsafe {
                    DeleteObject(*font);
                }
                *font = null_mut();
            }
        }
        for (i, (size, weight)) in [(12, 600), (10, 500), (9, 400), (11, 700)]
            .into_iter()
            .enumerate()
        {
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
                    if theme == Theme::Overlay {
                        NONANTIALIASED_QUALITY as u32
                    } else {
                        CLEARTYPE_QUALITY as u32
                    },
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
    wheel_remainder: i32,
    content_height: i32,
    window_shape: Option<PanelShape>,
    width: i32,
    height: i32,
    topmost: bool,
    settings: Arc<crate::settings::SharedSettings>,
    preferences: crate::settings::Settings,
    dialog: HWND,
    logos: Logos,
    tray: NOTIFYICONDATAW,
    taskbar_message: u32,
    smoke_test: bool,
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
    let window_width = themes::width(preferences.theme);
    let (window_x, window_y) = preferences.window_position.unwrap_or((40, 80));
    let instance = unsafe { GetModuleHandleW(null()) };
    let resource_icon = |id: usize| unsafe { LoadIconW(instance, id as _) };
    let mut app = Box::new(App {
        latest,
        snapshot: Snapshot::default(),
        gdi: GdiObjects::new(dpi)?,
        dpi,
        scroll: 0,
        wheel_remainder: 0,
        content_height: INITIAL_HEIGHT,
        window_shape: None,
        width: window_width,
        height: INITIAL_HEIGHT,
        topmost: preferences.topmost,
        settings,
        preferences,
        dialog: null_mut(),
        logos: Logos::new(dpi)?,
        tray: NOTIFYICONDATAW::default(),
        taskbar_message: unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) },
        smoke_test,
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
            }) | if app.preferences.theme == Theme::Overlay {
                WS_EX_LAYERED
            } else {
                0
            } | if app.topmost { WS_EX_TOPMOST } else { 0 },
            class.lpszClassName,
            w!("monitor2"),
            WS_POPUP,
            window_x,
            window_y,
            scale(window_width, dpi),
            scale(INITIAL_HEIGHT, dpi),
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
    let resources = app
        .gdi
        .fonts_for_theme(app.dpi, app.preferences.theme)
        .and_then(|()| Logos::new(app.dpi));
    match resources {
        Ok(logos) => app.logos = logos,
        Err(error) => {
            unsafe {
                DestroyWindow(hwnd);
                UnregisterClassW(class.lpszClassName, instance);
            }
            return Err(error);
        }
    }
    if let Err(error) = apply_window_theme(hwnd, app.preferences.theme) {
        unsafe {
            DestroyWindow(hwnd);
            UnregisterClassW(class.lpszClassName, instance);
        }
        return Err(error);
    }
    unsafe {
        // The restored position may be on a different-DPI monitor from the system.
        // No App borrow is held across this synchronous, reentrant call.
        SetWindowPos(
            hwnd,
            null_mut(),
            0,
            0,
            scale(app.width, app.dpi),
            scale(INITIAL_HEIGHT, app.dpi),
            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
        update_window_shape(hwnd, &mut *app);
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
        if SetTimer(hwnd, TIMER, app.preferences.interval_ms, None) == 0
            || (smoke_test && SetTimer(hwnd, SMOKE_TIMER, 6000, None) == 0)
        {
            DestroyWindow(hwnd);
            UnregisterClassW(class.lpszClassName, instance);
            return Err(last_error("SetTimer"));
        }
    }
    unsafe {
        refresh(hwnd, &mut *app);
        keep_window_on_screen(hwnd);
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
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
        WM_TIMER if wparam == SMOKE_TIMER => {
            unsafe {
                KillTimer(hwnd, SMOKE_TIMER);
                PostMessageW(hwnd, WM_CLOSE, 0, 0);
            }
            0
        }
        WM_TIMER if wparam == TIMER => {
            unsafe {
                refresh(hwnd, pointer);
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
        WM_SIZE => {
            unsafe {
                update_window_shape(hwnd, pointer);
            }
            0
        }
        WM_LBUTTONDOWN => {
            unsafe {
                windows_sys::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture();
                SendMessageW(hwnd, WM_NCLBUTTONDOWN, HTCAPTION as usize, 0);
            }
            0
        }
        WM_EXITSIZEMOVE => {
            unsafe {
                save_window_position(hwnd, pointer);
            }
            0
        }
        WM_QUERYENDSESSION => {
            unsafe {
                save_window_position(hwnd, pointer);
            }
            1
        }
        crate::settings_ui::CHANGED => {
            let preferences = unsafe { (*pointer).settings.get() };
            let topmost = preferences.topmost;
            unsafe {
                (*pointer).topmost = topmost;
                SetTimer(hwnd, TIMER, preferences.interval_ms, None);
                let theme_changed = (*pointer).preferences.theme != preferences.theme;
                (*pointer).preferences = preferences;
                if theme_changed {
                    (*pointer).scroll = 0;
                    (*pointer).wheel_remainder = 0;
                    let theme = (*pointer).preferences.theme;
                    let dpi = (*pointer).dpi;
                    if let Err(error) = (*pointer)
                        .gdi
                        .fonts_for_theme(dpi, theme)
                        .and_then(|()| apply_window_theme(hwnd, theme))
                    {
                        show_error(&error);
                        PostMessageW(hwnd, WM_CLOSE, 0, 0);
                    }
                }
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
                let steps = wheel_steps(&mut app.wheel_remainder, delta);
                app.scroll =
                    (app.scroll - steps * 48).clamp(0, (app.content_height - app.height).max(0));
                InvalidateRect(hwnd, null(), 0);
            }
            unsafe {
                update_window_shape(hwnd, pointer);
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
                if let Err(error) = (*pointer)
                    .gdi
                    .fonts_for_theme(dpi, (*pointer).preferences.theme)
                {
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
                save_window_position(hwnd, pointer);
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
                KillTimer(hwnd, SMOKE_TIMER);
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

unsafe fn save_window_position(hwnd: HWND, pointer: *mut App) {
    // SAFETY: live UI-thread window/App. GetWindowRect does not dispatch messages;
    // clone shared ownership before any error dialog can reenter the procedure.
    if unsafe { (*pointer).smoke_test } {
        return;
    }
    let mut rect = RECT::default();
    let result = if unsafe { GetWindowRect(hwnd, &mut rect) } == 0 {
        Err(last_error("GetWindowRect"))
    } else {
        let settings = unsafe { Arc::clone(&(*pointer).settings) };
        settings.save_window_position((rect.left, rect.top))
    };
    if let Err(error) = result {
        let error = wide(&error);
        unsafe {
            MessageBoxW(
                hwnd,
                error.as_ptr(),
                w!("ウィンドウ位置を保存できません"),
                MB_OK | MB_ICONERROR,
            );
        }
    }
}

fn position_in_work_area(rect: RECT, work: RECT) -> (i32, i32) {
    // Saturation also handles manually edited coordinates near i32 limits.
    let width = rect.right.saturating_sub(rect.left).max(0);
    let height = rect.bottom.saturating_sub(rect.top).max(0);
    (
        rect.left
            .clamp(work.left, work.right.saturating_sub(width).max(work.left)),
        rect.top
            .clamp(work.top, work.bottom.saturating_sub(height).max(work.top)),
    )
}

unsafe fn keep_window_on_screen(hwnd: HWND) {
    let mut rect = RECT::default();
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: stack output buffers and live window; no App borrow crosses SetWindowPos.
    if unsafe { GetWindowRect(hwnd, &mut rect) } == 0
        || unsafe { GetMonitorInfoW(MonitorFromRect(&rect, MONITOR_DEFAULTTONEAREST), &mut info) }
            == 0
    {
        return;
    }
    let (x, y) = position_in_work_area(rect, info.rcWork);
    if (x, y) != (rect.left, rect.top) {
        unsafe {
            SetWindowPos(
                hwnd,
                null_mut(),
                x,
                y,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }
}

fn apply_window_theme(hwnd: HWND, theme: Theme) -> Result<(), String> {
    // SAFETY: live UI-thread window. No App borrow crosses these reentrant APIs.
    let style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } as u32;
    let new_style = if theme == Theme::Overlay {
        style | WS_EX_LAYERED | WS_EX_NOACTIVATE
    } else {
        style & !(WS_EX_LAYERED | WS_EX_NOACTIVATE)
    };
    if style != new_style {
        unsafe {
            SetLastError(0);
        }
        if unsafe { SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_style as isize) } == 0
            && unsafe { GetLastError() } != 0
        {
            return Err(last_error("SetWindowLongPtr"));
        }
        unsafe {
            SetWindowPos(
                hwnd,
                null_mut(),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );
        }
    }
    if theme == Theme::Overlay
        && unsafe {
            SetLayeredWindowAttributes(hwnd, themes::overlay::TRANSPARENT_COLOR, 255, LWA_COLORKEY)
        } == 0
    {
        return Err(last_error("SetLayeredWindowAttributes"));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PanelShape {
    theme: Theme,
    width: i32,
    height: i32,
    dpi: u32,
    scroll: i32,
    cpu_height: i32,
    gpu_count: usize,
}

fn cpu_panel_height(rows: i32) -> i32 {
    102 + rows * CORE_ROW
}

impl PanelShape {
    fn region(self) -> Result<HRGN, String> {
        // SAFETY: all regions are owned locally until the union is returned. Each
        // temporary is deleted on success/failure; the caller owns the result.
        unsafe {
            if self.theme == Theme::Overlay {
                let region = CreateRectRgn(0, 0, self.width, self.height);
                return if region.is_null() {
                    Err(last_error("CreateRectRgn"))
                } else {
                    Ok(region)
                };
            }
            let region = CreateRectRgn(0, 0, 0, 0);
            if region.is_null() {
                return Err(last_error("CreateRectRgn"));
            }
            let diameter = scale(CORNER_DIAMETER, self.dpi);
            for i in 0..=self.gpu_count {
                let (top, height) = if i == 0 {
                    (0, self.cpu_height)
                } else {
                    (
                        self.cpu_height
                            + PANEL_GAP
                            + (i - 1) as i32 * (GPU_PANEL_HEIGHT + PANEL_GAP),
                        GPU_PANEL_HEIGHT,
                    )
                };
                let bottom = scale(top + height - self.scroll, self.dpi);
                let top = scale(top - self.scroll, self.dpi);
                if bottom <= 0 || top >= self.height {
                    continue;
                }
                let panel = CreateRoundRectRgn(0, top, self.width, bottom, diameter, diameter);
                if panel.is_null() {
                    DeleteObject(region.cast());
                    return Err(last_error("CreateRoundRectRgn"));
                }
                let combined = CombineRgn(region, region, panel, RGN_OR);
                DeleteObject(panel.cast());
                if combined == 0 {
                    DeleteObject(region.cast());
                    return Err(last_error("CombineRgn"));
                }
            }
            let viewport = CreateRectRgn(0, 0, self.width, self.height);
            if viewport.is_null() {
                DeleteObject(region.cast());
                return Err(last_error("CreateRectRgn"));
            }
            let clipped = CombineRgn(region, region, viewport, RGN_AND);
            DeleteObject(viewport.cast());
            if clipped == 0 {
                DeleteObject(region.cast());
                return Err(last_error("CombineRgn"));
            }
            Ok(region)
        }
    }
}

unsafe fn update_window_shape(hwnd: HWND, pointer: *mut App) {
    let mut rect = RECT::default();
    // SAFETY: this is our live borderless window; its client and window origins match.
    if unsafe { GetClientRect(hwnd, &mut rect) } == 0 {
        return;
    }
    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
    let shape = {
        // SAFETY: UI-thread App; this borrow ends before SetWindowRgn can reenter.
        let app = unsafe { &*pointer };
        PanelShape {
            theme: app.preferences.theme,
            width: rect.right,
            height: rect.bottom,
            dpi,
            scroll: app.scroll,
            cpu_height: cpu_panel_height(core_rows(displayed_cpus(app).len())),
            gpu_count: app.snapshot.gpus.len().max(1),
        }
    };
    if rect.right <= 0 || rect.bottom <= 0 || unsafe { (*pointer).window_shape } == Some(shape) {
        return;
    }
    let region = match shape.region() {
        Ok(region) => region,
        Err(error) => {
            eprintln!("{error}");
            return;
        }
    };
    // Record before SetWindowRgn, which can synchronously send window-position messages.
    // No App reference is held over that call. Success transfers region ownership to Windows.
    unsafe {
        (*pointer).window_shape = Some(shape);
    }
    if unsafe { SetWindowRgn(hwnd, region, 1) } == 0 {
        eprintln!("{}", last_error("SetWindowRgn"));
        unsafe {
            (*pointer).window_shape = None;
            DeleteObject(region.cast());
        }
    }
}

unsafe fn refresh(hwnd: HWND, pointer: *mut App) {
    let (resized, width, height, dpi) = {
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
        app.content_height =
            themes::content_height(app.preferences.theme, rows, app.snapshot.gpus.len());
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
        let width = themes::width(app.preferences.theme);
        let resized = height != app.height || width != app.width;
        app.width = width;
        app.height = height;
        app.scroll = app.scroll.clamp(0, (app.content_height - height).max(0));
        (resized, width, height, app.dpi)
    };
    if resized {
        unsafe {
            SetWindowPos(
                hwnd,
                null_mut(),
                0,
                0,
                scale(width, dpi),
                scale(height, dpi),
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
            keep_window_on_screen(hwnd);
        }
    }
    unsafe {
        update_window_shape(hwnd, pointer);
        InvalidateRect(hwnd, null(), 0);
    }
}
unsafe fn menu(hwnd: HWND, pointer: *mut App) {
    // A diagnostic smoke run must not open an editor beside the normal instance.
    if unsafe { (*pointer).smoke_test } {
        return;
    }
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
    let width = scale(app.width, app.dpi);
    let height = scale(app.height, app.dpi);
    let buffered = app.gdi.buffer(screen, width, height);
    let dc = if buffered { app.gdi.dc } else { screen };
    unsafe {
        SetBkMode(dc, TRANSPARENT as i32);
    }
    let old_font = unsafe { SelectObject(dc, app.gdi.fonts[1]) };
    let mut painter = Painter {
        theme: app.preferences.theme,
        dc,
        dpi: app.dpi,
        offset: app.scroll,
        fonts: app.gdi.fonts,
    };
    themes::draw(&mut painter, app);
    unsafe {
        SelectObject(dc, old_font);
        if buffered {
            BitBlt(screen, 0, 0, width, height, dc, 0, 0, SRCCOPY);
        }
        EndPaint(hwnd, &ps);
    }
}
struct Painter {
    theme: Theme,
    dc: HDC,
    dpi: u32,
    offset: i32,
    fonts: [HFONT; 4],
}
impl Painter {
    fn percentage_width(&mut self, font: usize) -> i32 {
        let mut size = SIZE::default();
        // SAFETY: select a live owned font into the paint DC and restore its old font.
        let measured = unsafe {
            let old = SelectObject(self.dc, self.fonts[font]);
            let ok = GetTextExtentPoint32W(self.dc, w!("100%"), 4, &mut size);
            SelectObject(self.dc, old);
            ok != 0
        };
        if measured {
            (size.cx * 96 + self.dpi as i32 - 1) / self.dpi as i32 + 1
        } else {
            32
        }
    }
    fn frame(&mut self, y: i32, height: i32) {
        themes::frame(self, y, height);
    }
    fn rect(&mut self, x: i32, y: i32, width: i32, height: i32, color: u32) {
        let rect = RECT {
            left: scale(x, self.dpi),
            top: scale(y - self.offset, self.dpi),
            right: scale(x + width, self.dpi),
            bottom: scale(y + height - self.offset, self.dpi),
        };
        // SAFETY: stock brush is borrowed; changing its color allocates no GDI objects.
        unsafe {
            SetDCBrushColor(self.dc, themes::color(self.theme, color));
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
            SetTextColor(self.dc, themes::color(self.theme, color));
            DrawTextW(
                self.dc,
                text.as_ptr(),
                -1,
                &mut rect,
                DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX,
            );
        }
    }
    fn icon(&mut self, x: i32, y: i32, size: i32, icon: HICON, label: &str) {
        themes::icon(self, x, y, size, icon, label);
    }
    fn memory(&mut self, y: i32, label: &str, memory: Option<(u64, u64, u64)>, shades: [u32; 2]) {
        for (x, title) in [(6, "Used"), (50, "Free"), (94, "Total")] {
            self.text(x, y, 40, 11, title, 2, MUTED);
        }
        let values = memory.map(|m| [m.0, m.1, m.2]);
        for (i, x) in [6, 50, 94].into_iter().enumerate() {
            let text = values.map_or("N/A".into(), |v| format!("{:.1}G", gib(v[i])));
            self.text(x, y + 11, 40, 12, &text, 1, TEXT);
        }
        self.text(6, y + 24, 28, 12, label, 2, MUTED);
        let usage = memory.and_then(|(u, _, t)| (t > 0).then_some(u as f64 * 100.0 / t as f64));
        self.bar(36, y + 28, WIDTH - 42, 6, usage, shades);
    }
    fn panel(&mut self, y: i32) {
        themes::panel(self, y);
    }
    fn bar(
        &mut self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        value: Option<f64>,
        shades: [u32; 2],
    ) {
        themes::bar(self, x, y, width, height, value, shades);
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

fn wheel_steps(remainder: &mut i32, delta: i32) -> i32 {
    *remainder += delta;
    let steps = *remainder / 120;
    *remainder %= 120;
    steps
}
#[cfg(test)]
mod review_tests {
    use super::*;
    #[test]
    fn panel_regions_exclude_gaps_and_follow_scrolling_at_each_dpi() {
        for dpi in [96, 120, 144, 192] {
            for rows in [0, 4, 8, 32] {
                for scroll in [0, 48, 144] {
                    let cpu_height = cpu_panel_height(rows);
                    let shape = PanelShape {
                        theme: Theme::Default,
                        width: scale(WIDTH, dpi),
                        height: scale(480, dpi),
                        dpi,
                        scroll,
                        cpu_height,
                        gpu_count: 2,
                    };
                    let region = shape.region().unwrap();
                    // SAFETY: test owns a live region and deletes it after querying.
                    unsafe {
                        let x = scale(WIDTH / 2, dpi);
                        for i in 0..2 {
                            let gap = cpu_height + i * (GPU_PANEL_HEIGHT + PANEL_GAP) - scroll;
                            for y in scale(gap, dpi).max(0)
                                ..scale(gap + PANEL_GAP, dpi).min(shape.height)
                            {
                                assert_eq!(
                                    PtInRegion(region, x, y),
                                    0,
                                    "gap at dpi={dpi}, rows={rows}, scroll={scroll}, y={y}"
                                );
                            }
                            let panel_center = scale(gap + PANEL_GAP + GPU_PANEL_HEIGHT / 2, dpi);
                            if (0..shape.height).contains(&panel_center) {
                                assert_ne!(PtInRegion(region, x, panel_center), 0);
                            }
                        }
                        assert_eq!(PtInRegion(region, x, -1), 0);
                        assert_eq!(PtInRegion(region, x, shape.height), 0);
                        assert_eq!(PtInRegion(region, shape.width, 30), 0);
                        DeleteObject(region.cast());
                    }
                }
            }
        }
    }
    #[test]
    fn restored_position_stays_in_work_area() {
        let work = RECT {
            left: -1920,
            top: -200,
            right: 0,
            bottom: 880,
        };
        let rect = |x: i32, y: i32| RECT {
            left: x,
            top: y,
            right: x.saturating_add(140),
            bottom: y.saturating_add(280),
        };
        assert_eq!(
            position_in_work_area(rect(-1800, -100), work),
            (-1800, -100)
        );
        assert_eq!(position_in_work_area(rect(4000, 2000), work), (-140, 600));
        assert_eq!(
            position_in_work_area(rect(i32::MIN, i32::MIN), work),
            (-1920, -200)
        );
        assert_eq!(
            position_in_work_area(rect(i32::MAX, i32::MAX), work),
            (0, 880)
        );
        let tiny = RECT {
            left: 0,
            top: 0,
            right: 100,
            bottom: 100,
        };
        assert_eq!(position_in_work_area(rect(20, 20), tiny), (0, 0));
    }
    #[test]
    fn fractional_wheel_input_accumulates_in_both_directions() {
        let mut remainder = 0;
        assert_eq!(wheel_steps(&mut remainder, 60), 0);
        assert_eq!(wheel_steps(&mut remainder, 60), 1);
        assert_eq!(wheel_steps(&mut remainder, -30), 0);
        assert_eq!(wheel_steps(&mut remainder, -90), -1);
        assert_eq!(remainder, 0);
        assert_eq!(wheel_steps(&mut remainder, 60), 0);
        assert_eq!(wheel_steps(&mut remainder, -60), 0);
        assert_eq!(remainder, 0);
    }
    #[test]
    fn percentages_fit_at_common_dpi_scales() {
        for dpi in [96, 120, 144, 192] {
            let mut objects = GdiObjects::new(dpi).unwrap();
            // SAFETY: private owned compatible DC, released by GdiObjects on scope exit.
            objects.dc = unsafe { CreateCompatibleDC(null_mut()) };
            assert!(!objects.dc.is_null());
            let mut painter = Painter {
                theme: Theme::Default,
                dc: objects.dc,
                dpi,
                offset: 0,
                fonts: objects.fonts,
            };
            for font in [2, 3] {
                let width = painter.percentage_width(font);
                for text in ["100%", "N/A"] {
                    let wide = wide(text);
                    let mut size = SIZE::default();
                    unsafe {
                        let old = SelectObject(objects.dc, objects.fonts[font]);
                        assert_ne!(
                            GetTextExtentPoint32W(
                                objects.dc,
                                wide.as_ptr(),
                                (wide.len() - 1) as i32,
                                &mut size
                            ),
                            0
                        );
                        SelectObject(objects.dc, old);
                    }
                    assert!(
                        scale(width, dpi) >= size.cx,
                        "dpi={dpi}, font={font}, text={text}"
                    );
                }
                assert!(width < 64 - 16 - 1, "core bar still has room at {dpi} DPI");
            }
        }
    }
}
