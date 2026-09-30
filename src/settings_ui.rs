use crate::{
    hardware::wide,
    settings::{Settings, SharedSettings},
};
use std::{
    ptr::{null, null_mut},
    sync::Arc,
};
use windows_sys::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::LibraryLoader::GetModuleHandleW,
        UI::{HiDpi::*, WindowsAndMessaging::*},
    },
    core::w,
};
pub const CHANGED: u32 = WM_APP + 2;
pub const CLOSED: u32 = WM_APP + 3;
const TOP: i32 = 101;
const START: i32 = 102;
const INTERVAL: i32 = 103;
const CORES: i32 = 104;
const PERCENT: i32 = 105;
const NUMBERS: i32 = 106;
const SAVE: usize = 201;
const CANCEL: usize = 202;
struct Dialog {
    shared: Arc<SharedSettings>,
    owner: HWND,
    dpi: u32,
    created: bool,
}

pub fn open(owner: HWND, shared: Arc<SharedSettings>) -> Result<HWND, String> {
    let instance = unsafe { GetModuleHandleW(null()) };
    let class = WNDCLASSW {
        lpfnWndProc: Some(proc),
        hInstance: instance,
        hIcon: unsafe { LoadIconW(instance, 1usize as _) },
        lpszClassName: w!("Monitor2Settings"),
        hCursor: unsafe { LoadCursorW(null_mut(), IDC_ARROW) },
        hbrBackground: (COLOR_BTNFACE + 1) as _,
        ..Default::default()
    };
    if unsafe { RegisterClassW(&class) } == 0 && unsafe { GetLastError() } != 1410 {
        return Err("設定ウィンドウを登録できません".into());
    }
    let dpi = unsafe { GetDpiForWindow(owner) }.max(96);
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: s(360, dpi),
        bottom: s(362, dpi),
    };
    let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU;
    unsafe {
        AdjustWindowRectExForDpi(&mut rect, style, 0, 0, dpi);
    }
    let mut owned_dialog = Box::new(Dialog {
        shared,
        owner,
        dpi,
        created: false,
    });
    let dialog: *mut Dialog = &mut *owned_dialog;
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_APPWINDOW,
            class.lpszClassName,
            w!("monitor2 設定"),
            style,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            rect.right - rect.left,
            rect.bottom - rect.top,
            owner,
            null_mut(),
            instance,
            dialog.cast(),
        )
    };
    if hwnd.is_null() {
        return Err("設定ウィンドウを作成できません".into());
    }
    // Transfer ownership only after successful creation.
    let dialog = Box::into_raw(owned_dialog);
    unsafe {
        (*dialog).created = true;
    }
    let value = unsafe { (*dialog).shared.get() };
    child(hwnd, "STATIC", "全般", 0, 16, 12, 320, 22, 0, dpi);
    let top = child(
        hwnd,
        "BUTTON",
        "常に手前に表示する",
        BS_AUTOCHECKBOX as u32,
        20,
        38,
        320,
        22,
        TOP,
        dpi,
    );
    let start = child(
        hwnd,
        "BUTTON",
        "Windows起動時に起動する",
        BS_AUTOCHECKBOX as u32,
        20,
        65,
        320,
        22,
        START,
        dpi,
    );
    child(hwnd, "STATIC", "監視項目", 0, 16, 106, 320, 22, 0, dpi);
    child(
        hwnd,
        "STATIC",
        "監視インターバル（秒）",
        0,
        20,
        136,
        210,
        22,
        0,
        dpi,
    );
    child(
        hwnd,
        "EDIT",
        &format!("{}", value.interval_ms as f64 / 1000.0),
        WS_BORDER | ES_AUTOHSCROLL as u32,
        246,
        134,
        86,
        24,
        INTERVAL,
        dpi,
    );
    child(hwnd, "STATIC", "コア表示", 0, 20, 172, 100, 22, 0, dpi);
    let cores = child(
        hwnd,
        "COMBOBOX",
        "",
        CBS_DROPDOWNLIST as u32 | WS_VSCROLL,
        138,
        170,
        194,
        150,
        CORES,
        dpi,
    );
    unsafe {
        SendMessageW(
            cores,
            CB_ADDSTRING,
            0,
            wide("物理コアのみ").as_ptr() as isize,
        );
        SendMessageW(
            cores,
            CB_ADDSTRING,
            0,
            wide("論理コアすべて").as_ptr() as isize,
        );
        SendMessageW(
            cores,
            CB_SETCURSEL,
            if value.physical_cores { 0 } else { 1 },
            0,
        );
    }
    let percent = child(
        hwnd,
        "BUTTON",
        "コア使用率（%）を表示する",
        BS_AUTOCHECKBOX as u32,
        20,
        208,
        320,
        22,
        PERCENT,
        dpi,
    );
    let numbers = child(
        hwnd,
        "BUTTON",
        "CPU番号（00〜）を表示する",
        BS_AUTOCHECKBOX as u32,
        20,
        236,
        320,
        22,
        NUMBERS,
        dpi,
    );
    child(
        hwnd,
        "STATIC",
        "設定は保存後すぐに反映されます。",
        0,
        20,
        274,
        320,
        22,
        0,
        dpi,
    );
    child(
        hwnd,
        "BUTTON",
        "保存",
        BS_DEFPUSHBUTTON as u32,
        168,
        320,
        78,
        26,
        SAVE as i32,
        dpi,
    );
    child(
        hwnd,
        "BUTTON",
        "キャンセル",
        0,
        254,
        320,
        86,
        26,
        CANCEL as i32,
        dpi,
    );
    unsafe {
        SendMessageW(top, BM_SETCHECK, value.topmost as usize, 0);
        SendMessageW(start, BM_SETCHECK, value.autostart as usize, 0);
        SendMessageW(percent, BM_SETCHECK, value.show_core_percent as usize, 0);
        SendMessageW(numbers, BM_SETCHECK, value.show_core_numbers as usize, 0);
        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);
    }
    Ok(hwnd)
}
#[allow(clippy::too_many_arguments)]
fn child(
    owner: HWND,
    class: &str,
    text: &str,
    style: u32,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    id: i32,
    dpi: u32,
) -> HWND {
    let class = wide(class);
    let text = wide(text);
    // SAFETY: terminated strings, parent owns child controls, font is a borrowed stock object.
    let hwnd = unsafe {
        CreateWindowExW(
            0,
            class.as_ptr(),
            text.as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | style,
            s(x, dpi),
            s(y, dpi),
            s(width, dpi),
            s(height, dpi),
            owner,
            id as usize as _,
            GetModuleHandleW(null()),
            null(),
        )
    };
    unsafe {
        SendMessageW(
            hwnd,
            WM_SETFONT,
            GetStockObject(DEFAULT_GUI_FONT) as usize,
            1,
        );
    }
    hwnd
}
fn checked(hwnd: HWND, id: i32) -> bool {
    unsafe { SendMessageW(GetDlgItem(hwnd, id), BM_GETCHECK, 0, 0) != 0 }
}
fn read(hwnd: HWND) -> Result<Settings, String> {
    let mut text = [0u16; 64];
    unsafe {
        GetDlgItemTextW(hwnd, INTERVAL, text.as_mut_ptr(), text.len() as i32);
    }
    let end = text.iter().position(|&c| c == 0).unwrap_or(text.len());
    let seconds = String::from_utf16_lossy(&text[..end])
        .trim()
        .parse::<f64>()
        .map_err(|_| "監視インターバルは数値で指定してください")?;
    if !seconds.is_finite() || !(0.25..=60.0).contains(&seconds) {
        return Err("監視インターバルは0.25〜60秒で指定してください".into());
    }
    let selected = unsafe { SendMessageW(GetDlgItem(hwnd, CORES), CB_GETCURSEL, 0, 0) };
    if selected != 0 && selected != 1 {
        return Err("コア表示を選択してください".into());
    }
    Ok(Settings {
        topmost: checked(hwnd, TOP),
        autostart: checked(hwnd, START),
        interval_ms: (seconds * 1000.0).round() as u32,
        physical_cores: selected == 0,
        show_core_percent: checked(hwnd, PERCENT),
        show_core_numbers: checked(hwnd, NUMBERS),
    })
}
unsafe extern "system" fn proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_NCCREATE {
        let create = unsafe { &*(lparam as *const CREATESTRUCTW) };
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
        }
    }
    let pointer = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut Dialog;
    if pointer.is_null() {
        return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
    }
    match message {
        WM_COMMAND if wparam & 0xffff == SAVE => {
            let result = read(hwnd).and_then(|s| unsafe { (*pointer).shared.apply(s) });
            match result {
                Ok(()) => {
                    let owner = unsafe { (*pointer).owner };
                    unsafe {
                        PostMessageW(owner, CHANGED, 0, 0);
                        DestroyWindow(hwnd);
                    }
                }
                Err(error) => {
                    let error = wide(&error);
                    unsafe {
                        MessageBoxW(
                            hwnd,
                            error.as_ptr(),
                            w!("設定を保存できません"),
                            MB_OK | MB_ICONERROR,
                        );
                    }
                }
            }
            0
        }
        WM_COMMAND if wparam & 0xffff == CANCEL => {
            unsafe {
                DestroyWindow(hwnd);
            }
            0
        }
        WM_CLOSE => {
            unsafe {
                DestroyWindow(hwnd);
            }
            0
        }
        WM_DPICHANGED => {
            let rect = unsafe { *(lparam as *const RECT) };
            // Keep the existing dialog controls consistent; recreate on the next open at the new DPI.
            unsafe {
                (*pointer).dpi = (wparam & 0xffff) as u32;
                SetWindowPos(
                    hwnd,
                    null_mut(),
                    rect.left,
                    rect.top,
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
            0
        }
        WM_NCDESTROY => unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            if !(*pointer).created {
                return DefWindowProcW(hwnd, message, wparam, lparam);
            }
            let dialog = Box::from_raw(pointer);
            PostMessageW(dialog.owner, CLOSED, 0, 0);
            drop(dialog);
            DefWindowProcW(hwnd, message, wparam, lparam)
        },
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}
fn s(value: i32, dpi: u32) -> i32 {
    (value * dpi as i32 + 48) / 96
}
