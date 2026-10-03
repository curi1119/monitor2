//! Selective mouse handling for a natively click-through layered window.
use super::*;
use std::cell::Cell;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_CONTROL};

pub(super) const MOVE_DRAG: u32 = WM_APP + 4;
pub(super) const END_DRAG: u32 = WM_APP + 5;
#[derive(Clone, Copy)]
struct State {
    hwnd: HWND,
    dc: HDC,
    region: HRGN,
    key: Option<u32>,
    drag_enabled: bool,
    enabled: bool,
    gesture: Gesture,
    target: Option<Position>,
    move_posted: bool,
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
            enabled: true,
            gesture: Gesture::default(),
            target: None,
            move_posted: false,
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
pub(super) fn reconfigure(hwnd: HWND, enabled: bool, drag_enabled: bool, theme: Theme) -> bool {
    if let Some(mut state) = STATE.get().filter(|s| s.hwnd == hwnd) {
        state.enabled = enabled;
        state.drag_enabled = drag_enabled;
        state.key = (theme == Theme::Overlay).then_some(themes::overlay::TRANSPARENT_COLOR);
        state.dc = null_mut();
        state.gesture.cancel();
        state.target = None;
        // Keep an owned DOWN until its UP even if settings disable the hook.
        let keep = enabled || state.gesture.owned || state.right_down;
        STATE.set(Some(state));
        return keep;
    }
    false
}
pub(super) fn take_move(hwnd: HWND) -> Option<(i32, i32)> {
    let mut state = STATE.get().filter(|s| s.hwnd == hwnd)?;
    state.move_posted = false;
    let result = state.target.take().map(|p| (p.x, p.y));
    STATE.set(Some(state));
    result
}
pub(super) fn move_window(hwnd: HWND) {
    if let Some((x, y)) = take_move(hwnd) {
        // SAFETY: UI thread owns this window; no App reference crosses the move.
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Position {
    x: i32,
    y: i32,
}
impl From<POINT> for Position {
    fn from(p: POINT) -> Self {
        Self { x: p.x, y: p.y }
    }
}
#[derive(Clone, Copy, Default)]
struct Gesture {
    owned: bool,
    anchor: Option<(Position, Position)>,
}
#[derive(Debug, PartialEq, Eq)]
enum DragAction {
    Pass,
    Consume,
    Move(Position),
    End(Option<Position>),
}
impl Gesture {
    fn begin(&mut self, cursor: Position, origin: Position) {
        self.owned = true;
        self.anchor = Some((cursor, origin));
    }
    fn cancel(&mut self) {
        self.anchor = None;
    }
    fn event(&mut self, message: u32, point: Position) -> DragAction {
        if !self.owned {
            return DragAction::Pass;
        }
        let target = self.anchor.map(|(cursor, origin)| Position {
            x: origin.x.saturating_add(point.x.saturating_sub(cursor.x)),
            y: origin.y.saturating_add(point.y.saturating_sub(cursor.y)),
        });
        match message {
            WM_LBUTTONDOWN => DragAction::Consume,
            WM_MOUSEMOVE => target.map_or(DragAction::Pass, DragAction::Move),
            WM_LBUTTONUP => {
                self.owned = false;
                self.anchor = None;
                DragAction::End(target)
            }
            _ => DragAction::Pass,
        }
    }
}
fn post_move(state: &mut State, target: Position) {
    state.target = Some(target);
    // Coalesce high-frequency motion; no allocations or unbounded posted queue.
    if !state.move_posted {
        state.move_posted = unsafe { PostMessageW(state.hwnd, MOVE_DRAG, 0, 0) } != 0;
    }
}
unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // SAFETY: Windows supplies MSLLHOOKSTRUCT for HC_ACTION. Copy state before
    // native calls; never borrow App across APIs. Only post UI work here.
    if code == HC_ACTION as i32
        && let Some(mut state) = STATE.get()
    {
        let message = wparam as u32;
        let data = unsafe { *(lparam as *const MSLLHOOKSTRUCT) };
        let point = Position::from(data.pt);
        match state.gesture.event(message, point) {
            DragAction::Consume => return 1,
            DragAction::Move(target) => {
                post_move(&mut state, target);
                STATE.set(Some(state));
                // Let the OS move the cursor; only the owned buttons are suppressed.
                return unsafe { CallNextHookEx(null_mut(), code, wparam, lparam) };
            }
            DragAction::End(target) => {
                if let Some(target) = target {
                    post_move(&mut state, target);
                }
                STATE.set(Some(state));
                unsafe {
                    PostMessageW(state.hwnd, END_DRAG, 0, 0);
                }
                return 1;
            }
            DragAction::Pass => {}
        }
        if message == WM_RBUTTONUP && state.right_down {
            state.right_down = false;
            STATE.set(Some(state));
            unsafe {
                PostMessageW(state.hwnd, WM_RBUTTONUP, 0, 0);
                PostMessageW(state.hwnd, END_DRAG, 0, 0);
            }
            return 1;
        }
        let hit = state.enabled
            && matches!(message, WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MOUSEWHEEL)
            && on_surface(state, data.pt);
        if hit
            && message == WM_LBUTTONDOWN
            && state.drag_enabled
            && unsafe { GetAsyncKeyState(VK_CONTROL as i32) } < 0
        {
            let mut rect = RECT::default();
            if unsafe { GetWindowRect(state.hwnd, &mut rect) } != 0 {
                state.gesture.begin(
                    point,
                    Position {
                        x: rect.left,
                        y: rect.top,
                    },
                );
                STATE.set(Some(state));
                return 1;
            }
        } else if hit && message == WM_RBUTTONDOWN {
            state.right_down = true;
            STATE.set(Some(state));
            return 1;
        } else if hit
            && message == WM_MOUSEWHEEL
            && unsafe { PostMessageW(state.hwnd, WM_MOUSEWHEEL, data.mouseData as usize, 0) } != 0
        {
            return 1;
        }
    }
    unsafe { CallNextHookEx(null_mut(), code, wparam, lparam) }
}
#[cfg(test)]
mod tests {
    use super::*;
    const START: Position = Position { x: 300, y: 250 };
    const ORIGIN: Position = Position { x: 200, y: 150 };
    #[test]
    fn drag_tracks_motion_and_owns_release_without_os_button_state() {
        let mut gesture = Gesture::default();
        assert_eq!(gesture.event(WM_LBUTTONUP, START), DragAction::Pass);
        gesture.begin(START, ORIGIN);
        assert_eq!(
            gesture.event(WM_MOUSEMOVE, Position { x: -50, y: 20 }),
            DragAction::Move(Position { x: -150, y: -80 })
        );
        assert_eq!(
            gesture.event(WM_LBUTTONUP, Position { x: -40, y: 25 }),
            DragAction::End(Some(Position { x: -140, y: -75 }))
        );
        assert_eq!(gesture.event(WM_MOUSEMOVE, START), DragAction::Pass);
        assert_eq!(gesture.event(WM_LBUTTONUP, START), DragAction::Pass);
    }
    #[test]
    fn short_click_and_cancelled_drag_do_not_leak_release() {
        let mut gesture = Gesture::default();
        gesture.begin(START, ORIGIN);
        assert_eq!(
            gesture.event(WM_LBUTTONUP, START),
            DragAction::End(Some(ORIGIN))
        );
        gesture.begin(START, ORIGIN);
        gesture.cancel();
        assert_eq!(gesture.event(WM_MOUSEMOVE, START), DragAction::Pass);
        assert_eq!(gesture.event(WM_LBUTTONUP, START), DragAction::End(None));
        assert!(!gesture.owned);
    }

    #[test]
    fn hooked_motion_moves_native_window_and_release_is_consumed() {
        unsafe extern "system" fn test_proc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
            if m == MOVE_DRAG {
                move_window(h);
                return 0;
            }
            unsafe { DefWindowProcW(h, m, w, l) }
        }
        // SAFETY: private invisible window on this test thread. No input is injected
        // and no user windows/settings are touched. Feed the callback owned motion.
        unsafe {
            let instance = GetModuleHandleW(null());
            let class = WNDCLASSW {
                lpfnWndProc: Some(test_proc),
                hInstance: instance,
                lpszClassName: w!("Monitor2DragTest"),
                ..Default::default()
            };
            assert_ne!(RegisterClassW(&class), 0);
            let hwnd = CreateWindowExW(
                WS_EX_NOACTIVATE,
                class.lpszClassName,
                w!("Drag test"),
                WS_POPUP,
                200,
                150,
                100,
                48,
                null_mut(),
                null_mut(),
                instance,
                null(),
            );
            assert!(!hwnd.is_null());
            let mut gesture = Gesture::default();
            gesture.begin(START, ORIGIN);
            STATE.set(Some(State {
                hwnd,
                dc: null_mut(),
                region: null_mut(),
                key: None,
                drag_enabled: true,
                enabled: true,
                gesture,
                target: None,
                move_posted: false,
                right_down: false,
            }));
            let mut data = MSLLHOOKSTRUCT {
                pt: POINT { x: 340, y: 280 },
                ..Default::default()
            };
            mouse_proc(
                HC_ACTION as i32,
                WM_MOUSEMOVE as usize,
                &data as *const _ as isize,
            );
            data.pt = POINT { x: 350, y: 290 };
            mouse_proc(
                HC_ACTION as i32,
                WM_MOUSEMOVE as usize,
                &data as *const _ as isize,
            );
            assert_eq!(
                mouse_proc(
                    HC_ACTION as i32,
                    WM_LBUTTONUP as usize,
                    &data as *const _ as isize
                ),
                1
            );
            let mut msg = MSG::default();
            let mut moves = 0;
            let mut ends = 0;
            while PeekMessageW(&mut msg, hwnd, MOVE_DRAG, END_DRAG, PM_REMOVE) != 0 {
                if msg.message == MOVE_DRAG {
                    moves += 1;
                }
                if msg.message == END_DRAG {
                    ends += 1;
                }
                DispatchMessageW(&msg);
            }
            assert_eq!((moves, ends), (1, 1));
            let mut rect = RECT::default();
            GetWindowRect(hwnd, &mut rect);
            assert_eq!((rect.left, rect.top), (250, 190));
            assert!(!STATE.get().unwrap().gesture.owned);
            let mut state = STATE.get().unwrap();
            state.gesture.begin(START, ORIGIN);
            STATE.set(Some(state));
            mouse_proc(
                HC_ACTION as i32,
                WM_MOUSEMOVE as usize,
                &data as *const _ as isize,
            );
            assert!(reconfigure(hwnd, false, false, Theme::Default));
            assert_eq!(
                mouse_proc(
                    HC_ACTION as i32,
                    WM_LBUTTONUP as usize,
                    &data as *const _ as isize
                ),
                1
            );
            while PeekMessageW(&mut msg, hwnd, MOVE_DRAG, END_DRAG, PM_REMOVE) != 0 {
                DispatchMessageW(&msg);
            }
            GetWindowRect(hwnd, &mut rect);
            assert_eq!((rect.left, rect.top), (250, 190));
            assert!(!reconfigure(hwnd, false, false, Theme::Default));
            STATE.set(None);
            DestroyWindow(hwnd);
            UnregisterClassW(class.lpszClassName, instance);
        }
    }
}
