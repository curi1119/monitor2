//! Selective mouse handling for a natively click-through layered window.
use super::*;
use std::cell::Cell;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_CONTROL};

pub(super) const BEGIN_DRAG: u32 = WM_APP + 4;
#[derive(Clone, Copy)]
struct State {
    hwnd: HWND,
    dc: HDC,
    region: HRGN,
    key: Option<u32>,
    drag_enabled: bool,
    suspended: bool,
    right_down: bool,
}
thread_local! { static STATE: Cell<Option<State>> = const { Cell::new(None) }; }

pub(super) struct MouseHook {
    hook: HHOOK,
    region: HRGN,
}
impl MouseHook {
    pub(super) fn new(hwnd: HWND, drag_enabled: bool, theme: Theme) -> Result<Self, String> {
        // SAFETY: only the installing UI thread accesses this state. The callback
        // receives native handles, never a reference to App. No other process is
        // injected and no input is recorded. Drop clears state before freeing handles.
        let region = unsafe { CreateRectRgn(0, 0, 0, 0) };
        if region.is_null() {
            return Err(last_error("CreateRectRgn"));
        }
        STATE.set(Some(State {
            hwnd,
            dc: null_mut(),
            region,
            key: (theme == Theme::Overlay).then_some(themes::overlay::TRANSPARENT_COLOR),
            drag_enabled,
            suspended: false,
            right_down: false,
        }));
        let hook = unsafe {
            SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), GetModuleHandleW(null()), 0)
        };
        if hook.is_null() {
            STATE.set(None);
            unsafe {
                DeleteObject(region.cast());
            }
            return Err(last_error("SetWindowsHookEx"));
        }
        update_region(hwnd);
        Ok(Self { hook, region })
    }
}
impl Drop for MouseHook {
    fn drop(&mut self) {
        STATE.set(None);
        // SAFETY: UI-thread owned hook/region; no callback can use cleared state.
        unsafe {
            UnhookWindowsHookEx(self.hook);
            DeleteObject(self.region.cast());
        }
    }
}
pub(super) fn update_region(hwnd: HWND) {
    if let Some(state) = STATE.get().filter(|s| s.hwnd == hwnd) {
        unsafe {
            GetWindowRgn(hwnd, state.region);
        }
    }
}
pub(super) fn surface(hwnd: HWND, dc: HDC) {
    if let Some(mut state) = STATE.get().filter(|s| s.hwnd == hwnd) {
        state.dc = dc;
        STATE.set(Some(state));
    }
}
pub(super) fn suspend(hwnd: HWND, suspended: bool) {
    if let Some(mut state) = STATE.get().filter(|s| s.hwnd == hwnd) {
        state.suspended = suspended;
        STATE.set(Some(state));
    }
}

fn on_surface(state: State, point: POINT) -> bool {
    // Only button/wheel events reach here: no per-mouse-move geometry work/allocation.
    // Conservative z-order check avoids intercepting clicks on covering windows.
    unsafe {
        let mut window = GetTopWindow(null_mut());
        let mut remaining = 256;
        while !window.is_null() && remaining > 0 {
            remaining -= 1;
            if window == state.hwnd {
                break;
            }
            let mut rect = RECT::default();
            if IsWindowVisible(window) != 0
                && IsIconic(window) == 0
                && GetWindowLongPtrW(window, GWL_EXSTYLE) as u32 & WS_EX_TRANSPARENT == 0
                && GetWindowRect(window, &mut rect) != 0
                && PtInRect(&rect, point) != 0
            {
                return false;
            }
            window = GetWindow(window, GW_HWNDNEXT);
        }
        if window != state.hwnd || IsWindowVisible(state.hwnd) == 0 || IsIconic(state.hwnd) != 0 {
            return false;
        }
        let mut rect = RECT::default();
        if GetWindowRect(state.hwnd, &mut rect) == 0 || PtInRect(&rect, point) == 0 {
            return false;
        }
        let (x, y) = (point.x - rect.left, point.y - rect.top);
        if PtInRegion(state.region, x, y) == 0 {
            return false;
        }
        if let Some(key) = state.key {
            if state.dc.is_null() {
                return false;
            }
            let pixel = GetPixel(state.dc, x, y);
            if pixel == key || pixel == CLR_INVALID {
                return false;
            }
        }
        true
    }
}
#[derive(Debug, PartialEq, Eq)]
enum Action {
    Pass,
    BeginDrag,
    RightDown,
    Menu,
    Wheel,
}
fn action(message: u32, control: bool, hit: bool, drag_enabled: bool, right_down: bool) -> Action {
    if message == WM_RBUTTONUP && right_down {
        Action::Menu
    } else if message == WM_RBUTTONDOWN && hit {
        Action::RightDown
    } else if message == WM_LBUTTONDOWN && hit && control && drag_enabled {
        Action::BeginDrag
    } else if message == WM_MOUSEWHEEL && hit {
        Action::Wheel
    } else {
        Action::Pass
    }
}
unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // SAFETY: Windows supplies MSLLHOOKSTRUCT for HC_ACTION. State is copied before
    // native calls; no Rust borrow crosses reentrant APIs. Callback does no I/O,
    // rendering or message dispatch: actions are posted to the normal UI loop.
    if code == HC_ACTION as i32
        && let Some(mut state) = STATE.get().filter(|s| !s.suspended)
    {
        let message = wparam as u32;
        let hit = matches!(message, WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MOUSEWHEEL)
            && on_surface(state, unsafe { (*(lparam as *const MSLLHOOKSTRUCT)).pt });
        let control =
            message == WM_LBUTTONDOWN && unsafe { GetAsyncKeyState(VK_CONTROL as i32) } < 0;
        match action(message, control, hit, state.drag_enabled, state.right_down) {
            Action::RightDown => {
                state.right_down = true;
                STATE.set(Some(state));
                return 1;
            }
            Action::Menu => {
                state.right_down = false;
                STATE.set(Some(state));
                unsafe {
                    PostMessageW(state.hwnd, WM_RBUTTONUP, 0, 0);
                }
                return 1;
            }
            Action::BeginDrag => {
                if unsafe { PostMessageW(state.hwnd, BEGIN_DRAG, 0, 0) } != 0 {
                    return 1;
                }
            }
            Action::Pass => {}
            Action::Wheel => {
                let data = unsafe { (*(lparam as *const MSLLHOOKSTRUCT)).mouseData };
                if unsafe { PostMessageW(state.hwnd, WM_MOUSEWHEEL, data as usize, 0) } != 0 {
                    return 1;
                }
            }
        }
    }
    unsafe { CallNextHookEx(null_mut(), code, wparam, lparam) }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn plain_left_clicks_pass_and_only_enabled_control_drags_are_captured() {
        for drag in [false, true] {
            assert_eq!(
                action(WM_LBUTTONDOWN, false, true, drag, false),
                Action::Pass
            );
            assert_eq!(action(WM_LBUTTONUP, true, true, drag, false), Action::Pass);
            assert_eq!(
                action(WM_LBUTTONDOWN, true, false, drag, false),
                Action::Pass
            );
            assert_eq!(
                action(WM_LBUTTONDOWN, true, true, drag, false),
                if drag {
                    Action::BeginDrag
                } else {
                    Action::Pass
                }
            );
            assert_eq!(
                action(WM_RBUTTONDOWN, false, true, drag, false),
                Action::RightDown
            );
            assert_eq!(
                action(WM_RBUTTONDOWN, false, false, drag, false),
                Action::Pass
            );
            assert_eq!(action(WM_RBUTTONUP, false, false, drag, true), Action::Menu);
            assert_eq!(action(WM_RBUTTONUP, false, true, drag, false), Action::Pass);
            assert_eq!(
                action(WM_MOUSEWHEEL, false, true, drag, false),
                Action::Wheel
            );
            assert_eq!(
                action(WM_MOUSEWHEEL, false, false, drag, false),
                Action::Pass
            );
        }
    }
}
