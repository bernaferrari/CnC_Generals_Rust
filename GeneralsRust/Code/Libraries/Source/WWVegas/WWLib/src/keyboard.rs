//! Keyboard input handling (ported from WWLib keyboard.cpp/h).

use crate::msgloop::windows_message_handler;
use crate::xmouse::with_mouse_cursor;

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};

#[cfg(not(target_os = "windows"))]
pub type HWND = usize;

#[cfg(not(target_os = "windows"))]
pub type WPARAM = usize;

#[cfg(not(target_os = "windows"))]
pub type LPARAM = isize;

#[cfg(target_os = "windows")]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyState, MapVirtualKeyA, ToAscii,
};

#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
    ClientToScreen, DefWindowProcW, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN,
    WM_LBUTTONUP, WM_MBUTTONDBLCLK, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_RBUTTONDBLCLK, WM_RBUTTONDOWN,
    WM_RBUTTONUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

pub const WWKEY_SHIFT_BIT: u16 = 0x0100;
pub const WWKEY_CTRL_BIT: u16 = 0x0200;
pub const WWKEY_ALT_BIT: u16 = 0x0400;
pub const WWKEY_RLS_BIT: u16 = 0x0800;
pub const WWKEY_VK_BIT: u16 = 0x1000;
pub const WWKEY_DBL_BIT: u16 = 0x2000;
pub const WWKEY_BTN_BIT: u16 = 0x8000;

pub struct WWKeyboardClass {
    pub mouse_qx: i32,
    pub mouse_qy: i32,
    key_state: [u8; 256],
    buffer: [u16; 256],
    head: usize,
    tail: usize,
}

impl WWKeyboardClass {
    pub fn new() -> Self {
        WWKeyboardClass {
            mouse_qx: 0,
            mouse_qy: 0,
            key_state: [0; 256],
            buffer: [0; 256],
            head: 0,
            tail: 0,
        }
    }

    pub fn check(&mut self) -> u16 {
        self.fill_buffer_from_system();
        if self.is_buffer_empty() {
            0
        } else {
            self.peek_element()
        }
    }

    pub fn get(&mut self) -> u16 {
        while self.check() == 0 {}
        self.buff_get()
    }

    pub fn put(&mut self, key: u16) -> bool {
        if !self.is_buffer_full() {
            self.put_element(key)
        } else {
            false
        }
    }

    pub fn clear(&mut self) {
        self.fill_buffer_from_system();
        self.head = self.tail;
        self.fill_buffer_from_system();
        self.head = self.tail;
    }

    pub fn to_ascii(&mut self, key: u16) -> char {
        #[cfg(not(target_os = "windows"))]
        {
            let keycode = (key & 0xFF) as u8;
            return keycode as char;
        }
        #[cfg(target_os = "windows")]
        {
            if (key & WWKEY_RLS_BIT) != 0 {
                return '\0';
            }

            if (key & WWKEY_SHIFT_BIT) != 0 {
                self.key_state[VK_SHIFT as usize] = 0x80;
            }
            if (key & WWKEY_CTRL_BIT) != 0 {
                self.key_state[VK_CONTROL as usize] = 0x80;
            }
            if (key & WWKEY_ALT_BIT) != 0 {
                self.key_state[VK_MENU as usize] = 0x80;
            }

            let mut buffer: [u16; 10] = [0; 10];
            let scancode = unsafe { MapVirtualKeyA((key & 0xFF) as u32, 0) };
            let result = unsafe {
                ToAscii(
                    (key & 0xFF) as u32,
                    scancode,
                    self.key_state.as_ptr(),
                    buffer.as_mut_ptr(),
                    0,
                )
            };

            if (key & WWKEY_SHIFT_BIT) != 0 {
                self.key_state[VK_SHIFT as usize] = 0;
            }
            if (key & WWKEY_CTRL_BIT) != 0 {
                self.key_state[VK_CONTROL as usize] = 0;
            }
            if (key & WWKEY_ALT_BIT) != 0 {
                self.key_state[VK_MENU as usize] = 0;
            }

            if result != 1 {
                return '\0';
            }

            (buffer[0] as u8) as char
        }
    }

    pub fn down(&self, key: u16) -> bool {
        #[cfg(target_os = "windows")]
        {
            unsafe { GetAsyncKeyState((key & 0xFF) as i32) != 0 }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = key;
            false
        }
    }

    pub fn message_handler(
        &mut self,
        window: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> bool {
        #[cfg(target_os = "windows")]
        {
            let mut point = POINT {
                x: loword(lparam.0),
                y: hiword(lparam.0),
            };
            unsafe {
                ClientToScreen(window, &mut point);
            }
            let mut x = point.x;
            let mut y = point.y;
            let _ = with_mouse_cursor(|cursor| cursor.convert_coordinate(&mut x, &mut y));

            let mut processed = false;
            match message {
                WM_SYSKEYDOWN | WM_KEYDOWN => {
                    if wparam.0 as u32 == VK_SCROLL {
                        stop_execution();
                    } else {
                        self.put_key_message(wparam.0 as u16, false);
                    }
                    processed = true;
                }
                WM_SYSKEYUP | WM_KEYUP => {
                    self.put_key_message(wparam.0 as u16, true);
                    processed = true;
                }
                WM_LBUTTONDOWN => {
                    self.put_mouse_message(VK_LBUTTON as u16, x, y, false);
                    processed = true;
                }
                WM_LBUTTONUP => {
                    self.put_mouse_message(VK_LBUTTON as u16, x, y, true);
                    processed = true;
                }
                WM_LBUTTONDBLCLK => {
                    self.put_mouse_message(VK_LBUTTON as u16, x, y, false);
                    self.put_mouse_message(VK_LBUTTON as u16, x, y, true);
                    self.put_mouse_message(VK_LBUTTON as u16, x, y, false);
                    self.put_mouse_message(VK_LBUTTON as u16, x, y, true);
                    processed = true;
                }
                WM_MBUTTONDOWN => {
                    self.put_mouse_message(VK_MBUTTON as u16, x, y, false);
                    processed = true;
                }
                WM_MBUTTONUP => {
                    self.put_mouse_message(VK_MBUTTON as u16, x, y, true);
                    processed = true;
                }
                WM_MBUTTONDBLCLK => {
                    self.put_mouse_message(VK_MBUTTON as u16, x, y, false);
                    self.put_mouse_message(VK_MBUTTON as u16, x, y, true);
                    self.put_mouse_message(VK_MBUTTON as u16, x, y, false);
                    self.put_mouse_message(VK_MBUTTON as u16, x, y, true);
                    processed = true;
                }
                WM_RBUTTONDOWN => {
                    self.put_mouse_message(VK_RBUTTON as u16, x, y, false);
                    processed = true;
                }
                WM_RBUTTONUP => {
                    self.put_mouse_message(VK_RBUTTON as u16, x, y, true);
                    processed = true;
                }
                WM_RBUTTONDBLCLK => {
                    self.put_mouse_message(VK_RBUTTON as u16, x, y, false);
                    self.put_mouse_message(VK_RBUTTON as u16, x, y, true);
                    self.put_mouse_message(VK_RBUTTON as u16, x, y, false);
                    self.put_mouse_message(VK_RBUTTON as u16, x, y, true);
                    processed = true;
                }
                _ => {}
            }

            if processed {
                unsafe {
                    DefWindowProcW(window, message, wparam, lparam);
                }
                return true;
            }
            false
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (window, message, wparam, lparam);
            false
        }
    }

    fn buff_get(&mut self) -> u16 {
        while self.check() == 0 {}
        let temp = self.fetch_element();
        if Self::is_mouse_key(temp) {
            self.mouse_qx = self.fetch_element() as i32;
            self.mouse_qy = self.fetch_element() as i32;
        }
        temp
    }

    fn fetch_element(&mut self) -> u16 {
        let mut val = 0u16;
        if self.head != self.tail {
            val = self.buffer[self.head];
            self.head = (self.head + 1) % self.buffer.len();
        }
        val
    }

    fn peek_element(&self) -> u16 {
        if !self.is_buffer_empty() {
            self.buffer[self.head]
        } else {
            0
        }
    }

    fn put_element(&mut self, val: u16) -> bool {
        if !self.is_buffer_full() {
            let temp = (self.tail + 1) % self.buffer.len();
            self.buffer[self.tail] = val;
            self.tail = temp;
            true
        } else {
            false
        }
    }

    fn is_buffer_full(&self) -> bool {
        (self.tail + 1) % self.buffer.len() == self.head
    }

    fn is_buffer_empty(&self) -> bool {
        self.head == self.tail
    }

    fn available_buffer_room(&self) -> i32 {
        let mut avail = 0i32;
        if self.head == self.tail {
            avail = self.buffer.len() as i32;
        }
        if self.head < self.tail {
            avail = (self.tail - self.head) as i32;
        }
        if self.head > self.tail {
            avail = (self.tail + self.buffer.len() - self.head) as i32;
        }
        avail
    }

    fn fill_buffer_from_system(&mut self) {
        if !self.is_buffer_full() {
            windows_message_handler();
        }
    }

    fn put_key_message(&mut self, mut vk_key: u16, release: bool) -> bool {
        if !Self::is_mouse_key(vk_key) {
            #[cfg(target_os = "windows")]
            {
                unsafe {
                    if (GetKeyState(VK_SHIFT as i32) & 0x8000) != 0
                        || (GetKeyState(VK_CAPITAL as i32) & 0x0008) != 0
                        || (GetKeyState(VK_NUMLOCK as i32) & 0x0008) != 0
                    {
                        vk_key |= WWKEY_SHIFT_BIT;
                    }
                    if (GetKeyState(VK_CONTROL as i32) & 0x8000) != 0 {
                        vk_key |= WWKEY_CTRL_BIT;
                    }
                    if (GetKeyState(VK_MENU as i32) & 0x8000) != 0 {
                        vk_key |= WWKEY_ALT_BIT;
                    }
                }
            }
        }

        if release {
            vk_key |= WWKEY_RLS_BIT;
        }

        self.put(vk_key)
    }

    fn put_mouse_message(&mut self, vk_key: u16, x: i32, y: i32, release: bool) -> bool {
        if self.available_buffer_room() >= 3 && Self::is_mouse_key(vk_key) {
            self.put_key_message(vk_key, release);
            self.put(x as u16);
            self.put(y as u16);
            true
        } else {
            false
        }
    }

    fn is_mouse_key(key: u16) -> bool {
        let key = key & 0xFF;
        key == VK_LBUTTON as u16 || key == VK_MBUTTON as u16 || key == VK_RBUTTON as u16
    }
}

pub struct KeyboardClass {
    pub is_library: bool,
    inner: WWKeyboardClass,
}

impl KeyboardClass {
    pub fn new() -> Self {
        KeyboardClass {
            is_library: true,
            inner: WWKeyboardClass::new(),
        }
    }

    pub fn get(&mut self) -> u16 {
        self.inner.get()
    }

    pub fn check(&mut self) -> u16 {
        self.inner.check()
    }

    pub fn to_ascii(&mut self, key: u16) -> u16 {
        self.inner.to_ascii(key) as u16
    }

    pub fn clear(&mut self) {
        self.inner.clear();
    }

    pub fn down(&self, key: u16) -> bool {
        self.inner.down(key)
    }

    pub fn mouse_x(&self) -> i32 {
        self.inner.mouse_qx
    }

    pub fn mouse_y(&self) -> i32 {
        self.inner.mouse_qy
    }
}

#[inline]
fn loword(value: isize) -> i32 {
    (value & 0xFFFF) as u16 as i32
}

#[inline]
fn hiword(value: isize) -> i32 {
    ((value >> 16) & 0xFFFF) as u16 as i32
}

fn stop_execution() {}

// Virtual key constants (subset mirrors full header list).
pub const VK_NONE: u32 = 0x00;
pub const VK_LBUTTON: u32 = 0x01;
pub const VK_RBUTTON: u32 = 0x02;
pub const VK_CANCEL: u32 = 0x03;
pub const VK_MBUTTON: u32 = 0x04;
pub const VK_BACK: u32 = 0x08;
pub const VK_TAB: u32 = 0x09;
pub const VK_CLEAR: u32 = 0x0C;
pub const VK_RETURN: u32 = 0x0D;
pub const VK_SHIFT: u32 = 0x10;
pub const VK_CONTROL: u32 = 0x11;
pub const VK_MENU: u32 = 0x12;
pub const VK_PAUSE: u32 = 0x13;
pub const VK_CAPITAL: u32 = 0x14;
pub const VK_ESCAPE: u32 = 0x1B;
pub const VK_SPACE: u32 = 0x20;
pub const VK_PRIOR: u32 = 0x21;
pub const VK_NEXT: u32 = 0x22;
pub const VK_END: u32 = 0x23;
pub const VK_HOME: u32 = 0x24;
pub const VK_LEFT: u32 = 0x25;
pub const VK_UP: u32 = 0x26;
pub const VK_RIGHT: u32 = 0x27;
pub const VK_DOWN: u32 = 0x28;
pub const VK_SELECT: u32 = 0x29;
pub const VK_PRINT: u32 = 0x2A;
pub const VK_EXECUTE: u32 = 0x2B;
pub const VK_SNAPSHOT: u32 = 0x2C;
pub const VK_INSERT: u32 = 0x2D;
pub const VK_DELETE: u32 = 0x2E;
pub const VK_HELP: u32 = 0x2F;
pub const VK_0: u32 = 0x30;
pub const VK_1: u32 = 0x31;
pub const VK_2: u32 = 0x32;
pub const VK_3: u32 = 0x33;
pub const VK_4: u32 = 0x34;
pub const VK_5: u32 = 0x35;
pub const VK_6: u32 = 0x36;
pub const VK_7: u32 = 0x37;
pub const VK_8: u32 = 0x38;
pub const VK_9: u32 = 0x39;
pub const VK_A: u32 = 0x41;
pub const VK_B: u32 = 0x42;
pub const VK_C: u32 = 0x43;
pub const VK_D: u32 = 0x44;
pub const VK_E: u32 = 0x45;
pub const VK_F: u32 = 0x46;
pub const VK_G: u32 = 0x47;
pub const VK_H: u32 = 0x48;
pub const VK_I: u32 = 0x49;
pub const VK_J: u32 = 0x4A;
pub const VK_K: u32 = 0x4B;
pub const VK_L: u32 = 0x4C;
pub const VK_M: u32 = 0x4D;
pub const VK_N: u32 = 0x4E;
pub const VK_O: u32 = 0x4F;
pub const VK_P: u32 = 0x50;
pub const VK_Q: u32 = 0x51;
pub const VK_R: u32 = 0x52;
pub const VK_S: u32 = 0x53;
pub const VK_T: u32 = 0x54;
pub const VK_U: u32 = 0x55;
pub const VK_V: u32 = 0x56;
pub const VK_W: u32 = 0x57;
pub const VK_X: u32 = 0x58;
pub const VK_Y: u32 = 0x59;
pub const VK_Z: u32 = 0x5A;
pub const VK_NUMPAD0: u32 = 0x60;
pub const VK_NUMPAD1: u32 = 0x61;
pub const VK_NUMPAD2: u32 = 0x62;
pub const VK_NUMPAD3: u32 = 0x63;
pub const VK_NUMPAD4: u32 = 0x64;
pub const VK_NUMPAD5: u32 = 0x65;
pub const VK_NUMPAD6: u32 = 0x66;
pub const VK_NUMPAD7: u32 = 0x67;
pub const VK_NUMPAD8: u32 = 0x68;
pub const VK_NUMPAD9: u32 = 0x69;
pub const VK_MULTIPLY: u32 = 0x6A;
pub const VK_ADD: u32 = 0x6B;
pub const VK_SEPARATOR: u32 = 0x6C;
pub const VK_SUBTRACT: u32 = 0x6D;
pub const VK_DECIMAL: u32 = 0x6E;
pub const VK_DIVIDE: u32 = 0x6F;
pub const VK_F1: u32 = 0x70;
pub const VK_F2: u32 = 0x71;
pub const VK_F3: u32 = 0x72;
pub const VK_F4: u32 = 0x73;
pub const VK_F5: u32 = 0x74;
pub const VK_F6: u32 = 0x75;
pub const VK_F7: u32 = 0x76;
pub const VK_F8: u32 = 0x77;
pub const VK_F9: u32 = 0x78;
pub const VK_F10: u32 = 0x79;
pub const VK_F11: u32 = 0x7A;
pub const VK_F12: u32 = 0x7B;
pub const VK_F13: u32 = 0x7C;
pub const VK_F14: u32 = 0x7D;
pub const VK_F15: u32 = 0x7E;
pub const VK_F16: u32 = 0x7F;
pub const VK_F17: u32 = 0x80;
pub const VK_F18: u32 = 0x81;
pub const VK_F19: u32 = 0x82;
pub const VK_F20: u32 = 0x83;
pub const VK_F21: u32 = 0x84;
pub const VK_F22: u32 = 0x85;
pub const VK_F23: u32 = 0x86;
pub const VK_F24: u32 = 0x87;
pub const VK_NUMLOCK: u32 = 0x90;
pub const VK_SCROLL: u32 = 0x91;
pub const VK_NONE_BA: u32 = 0xBA;
pub const VK_NONE_BB: u32 = 0xBB;
pub const VK_NONE_BC: u32 = 0xBC;
pub const VK_NONE_BD: u32 = 0xBD;
pub const VK_NONE_BE: u32 = 0xBE;
pub const VK_NONE_BF: u32 = 0xBF;
pub const VK_NONE_C0: u32 = 0xC0;
pub const VK_NONE_DB: u32 = 0xDB;
pub const VK_NONE_DC: u32 = 0xDC;
pub const VK_NONE_DD: u32 = 0xDD;
pub const VK_NONE_DE: u32 = 0xDE;

pub const VK_UPLEFT: u32 = VK_HOME;
pub const VK_UPRIGHT: u32 = VK_PRIOR;
pub const VK_DOWNLEFT: u32 = VK_END;
pub const VK_DOWNRIGHT: u32 = VK_NEXT;
pub const VK_ALT: u32 = VK_MENU;

pub type KeyASCIIType = u16;
pub type KeyNumType = u16;

pub const KA_NONE: KeyASCIIType = 0;
pub const KA_SPACE: KeyASCIIType = 32;
pub const KA_ESC: KeyASCIIType = VK_ESCAPE as KeyASCIIType;
pub const KA_RETURN: KeyASCIIType = VK_RETURN as KeyASCIIType;
pub const KA_BACKSPACE: KeyASCIIType = VK_BACK as KeyASCIIType;
pub const KA_TAB: KeyASCIIType = VK_TAB as KeyASCIIType;
pub const KA_DELETE: KeyASCIIType = VK_DELETE as KeyASCIIType;
pub const KA_INSERT: KeyASCIIType = VK_INSERT as KeyASCIIType;
pub const KA_PGDN: KeyASCIIType = VK_NEXT as KeyASCIIType;
pub const KA_DOWN: KeyASCIIType = VK_DOWN as KeyASCIIType;
pub const KA_END: KeyASCIIType = VK_END as KeyASCIIType;
pub const KA_RIGHT: KeyASCIIType = VK_RIGHT as KeyASCIIType;
pub const KA_KEYPAD5: KeyASCIIType = VK_SELECT as KeyASCIIType;
pub const KA_LEFT: KeyASCIIType = VK_LEFT as KeyASCIIType;
pub const KA_PGUP: KeyASCIIType = VK_PRIOR as KeyASCIIType;
pub const KA_UP: KeyASCIIType = VK_UP as KeyASCIIType;
pub const KA_HOME: KeyASCIIType = VK_HOME as KeyASCIIType;
pub const KA_F1: KeyASCIIType = VK_F1 as KeyASCIIType;
pub const KA_F2: KeyASCIIType = VK_F2 as KeyASCIIType;
pub const KA_F3: KeyASCIIType = VK_F3 as KeyASCIIType;
pub const KA_F4: KeyASCIIType = VK_F4 as KeyASCIIType;
pub const KA_F5: KeyASCIIType = VK_F5 as KeyASCIIType;
pub const KA_F6: KeyASCIIType = VK_F6 as KeyASCIIType;
pub const KA_F7: KeyASCIIType = VK_F7 as KeyASCIIType;
pub const KA_F8: KeyASCIIType = VK_F8 as KeyASCIIType;
pub const KA_F9: KeyASCIIType = VK_F9 as KeyASCIIType;
pub const KA_F10: KeyASCIIType = VK_F10 as KeyASCIIType;
pub const KA_F11: KeyASCIIType = VK_F11 as KeyASCIIType;
pub const KA_F12: KeyASCIIType = VK_F12 as KeyASCIIType;
pub const KA_LMOUSE: KeyASCIIType = VK_LBUTTON as KeyASCIIType;
pub const KA_RMOUSE: KeyASCIIType = VK_RBUTTON as KeyASCIIType;
pub const KA_SHIFT_BIT: KeyASCIIType = WWKEY_SHIFT_BIT;
pub const KA_CTRL_BIT: KeyASCIIType = WWKEY_CTRL_BIT;
pub const KA_ALT_BIT: KeyASCIIType = WWKEY_ALT_BIT;
pub const KA_RLSE_BIT: KeyASCIIType = WWKEY_RLS_BIT;

pub const KN_NONE: KeyNumType = 0;
pub const KN_0: KeyNumType = VK_0 as KeyNumType;
pub const KN_1: KeyNumType = VK_1 as KeyNumType;
pub const KN_2: KeyNumType = VK_2 as KeyNumType;
pub const KN_3: KeyNumType = VK_3 as KeyNumType;
pub const KN_4: KeyNumType = VK_4 as KeyNumType;
pub const KN_5: KeyNumType = VK_5 as KeyNumType;
pub const KN_6: KeyNumType = VK_6 as KeyNumType;
pub const KN_7: KeyNumType = VK_7 as KeyNumType;
pub const KN_8: KeyNumType = VK_8 as KeyNumType;
pub const KN_9: KeyNumType = VK_9 as KeyNumType;
pub const KN_A: KeyNumType = VK_A as KeyNumType;
pub const KN_B: KeyNumType = VK_B as KeyNumType;
pub const KN_C: KeyNumType = VK_C as KeyNumType;
pub const KN_D: KeyNumType = VK_D as KeyNumType;
pub const KN_E: KeyNumType = VK_E as KeyNumType;
pub const KN_F: KeyNumType = VK_F as KeyNumType;
pub const KN_G: KeyNumType = VK_G as KeyNumType;
pub const KN_H: KeyNumType = VK_H as KeyNumType;
pub const KN_I: KeyNumType = VK_I as KeyNumType;
pub const KN_J: KeyNumType = VK_J as KeyNumType;
pub const KN_K: KeyNumType = VK_K as KeyNumType;
pub const KN_L: KeyNumType = VK_L as KeyNumType;
pub const KN_M: KeyNumType = VK_M as KeyNumType;
pub const KN_N: KeyNumType = VK_N as KeyNumType;
pub const KN_O: KeyNumType = VK_O as KeyNumType;
pub const KN_P: KeyNumType = VK_P as KeyNumType;
pub const KN_Q: KeyNumType = VK_Q as KeyNumType;
pub const KN_R: KeyNumType = VK_R as KeyNumType;
pub const KN_S: KeyNumType = VK_S as KeyNumType;
pub const KN_T: KeyNumType = VK_T as KeyNumType;
pub const KN_U: KeyNumType = VK_U as KeyNumType;
pub const KN_V: KeyNumType = VK_V as KeyNumType;
pub const KN_W: KeyNumType = VK_W as KeyNumType;
pub const KN_X: KeyNumType = VK_X as KeyNumType;
pub const KN_Y: KeyNumType = VK_Y as KeyNumType;
pub const KN_Z: KeyNumType = VK_Z as KeyNumType;
pub const KN_BACKSLASH: KeyNumType = VK_NONE_DC as KeyNumType;
pub const KN_BACKSPACE: KeyNumType = VK_BACK as KeyNumType;
pub const KN_CAPSLOCK: KeyNumType = VK_CAPITAL as KeyNumType;
pub const KN_CENTER: KeyNumType = VK_CLEAR as KeyNumType;
pub const KN_COMMA: KeyNumType = VK_NONE_BC as KeyNumType;
pub const KN_DELETE: KeyNumType = VK_DELETE as KeyNumType;
pub const KN_DOWN: KeyNumType = VK_DOWN as KeyNumType;
pub const KN_DOWNLEFT: KeyNumType = VK_END as KeyNumType;
pub const KN_DOWNRIGHT: KeyNumType = VK_NEXT as KeyNumType;
pub const KN_END: KeyNumType = VK_END as KeyNumType;
pub const KN_EQUAL: KeyNumType = VK_NONE_BB as KeyNumType;
pub const KN_ESC: KeyNumType = VK_ESCAPE as KeyNumType;
pub const KN_E_DELETE: KeyNumType = VK_DELETE as KeyNumType;
pub const KN_E_DOWN: KeyNumType = VK_NUMPAD2 as KeyNumType;
pub const KN_E_END: KeyNumType = VK_NUMPAD1 as KeyNumType;
pub const KN_E_HOME: KeyNumType = VK_NUMPAD7 as KeyNumType;
pub const KN_E_INSERT: KeyNumType = VK_INSERT as KeyNumType;
pub const KN_E_LEFT: KeyNumType = VK_NUMPAD4 as KeyNumType;
pub const KN_E_PGDN: KeyNumType = VK_NUMPAD3 as KeyNumType;
pub const KN_E_PGUP: KeyNumType = VK_NUMPAD9 as KeyNumType;
pub const KN_E_RIGHT: KeyNumType = VK_NUMPAD6 as KeyNumType;
pub const KN_E_UP: KeyNumType = VK_NUMPAD8 as KeyNumType;
pub const KN_F1: KeyNumType = VK_F1 as KeyNumType;
pub const KN_F2: KeyNumType = VK_F2 as KeyNumType;
pub const KN_F3: KeyNumType = VK_F3 as KeyNumType;
pub const KN_F4: KeyNumType = VK_F4 as KeyNumType;
pub const KN_F5: KeyNumType = VK_F5 as KeyNumType;
pub const KN_F6: KeyNumType = VK_F6 as KeyNumType;
pub const KN_F7: KeyNumType = VK_F7 as KeyNumType;
pub const KN_F8: KeyNumType = VK_F8 as KeyNumType;
pub const KN_F9: KeyNumType = VK_F9 as KeyNumType;
pub const KN_F10: KeyNumType = VK_F10 as KeyNumType;
pub const KN_F11: KeyNumType = VK_F11 as KeyNumType;
pub const KN_F12: KeyNumType = VK_F12 as KeyNumType;
pub const KN_GRAVE: KeyNumType = VK_NONE_C0 as KeyNumType;
pub const KN_HOME: KeyNumType = VK_HOME as KeyNumType;
pub const KN_INSERT: KeyNumType = VK_INSERT as KeyNumType;
pub const KN_KEYPAD_ASTERISK: KeyNumType = VK_MULTIPLY as KeyNumType;
pub const KN_KEYPAD_MINUS: KeyNumType = VK_SUBTRACT as KeyNumType;
pub const KN_KEYPAD_PLUS: KeyNumType = VK_ADD as KeyNumType;
pub const KN_KEYPAD_RETURN: KeyNumType = VK_RETURN as KeyNumType;
pub const KN_KEYPAD_SLASH: KeyNumType = VK_DIVIDE as KeyNumType;
pub const KN_LALT: KeyNumType = VK_MENU as KeyNumType;
pub const KN_LBRACKET: KeyNumType = VK_NONE_DB as KeyNumType;
pub const KN_LCTRL: KeyNumType = VK_CONTROL as KeyNumType;
pub const KN_LEFT: KeyNumType = VK_LEFT as KeyNumType;
pub const KN_LMOUSE: KeyNumType = VK_LBUTTON as KeyNumType;
pub const KN_LSHIFT: KeyNumType = VK_SHIFT as KeyNumType;
pub const KN_MINUS: KeyNumType = VK_NONE_BD as KeyNumType;
pub const KN_NUMLOCK: KeyNumType = VK_NUMLOCK as KeyNumType;
pub const KN_PAUSE: KeyNumType = VK_PAUSE as KeyNumType;
pub const KN_PERIOD: KeyNumType = VK_NONE_BE as KeyNumType;
pub const KN_PGDN: KeyNumType = VK_NEXT as KeyNumType;
pub const KN_PGUP: KeyNumType = VK_PRIOR as KeyNumType;
pub const KN_PRNTSCRN: KeyNumType = VK_PRINT as KeyNumType;
pub const KN_RALT: KeyNumType = VK_MENU as KeyNumType;
pub const KN_RBRACKET: KeyNumType = VK_NONE_DD as KeyNumType;
pub const KN_RCTRL: KeyNumType = VK_CONTROL as KeyNumType;
pub const KN_RETURN: KeyNumType = VK_RETURN as KeyNumType;
pub const KN_RIGHT: KeyNumType = VK_RIGHT as KeyNumType;
pub const KN_RMOUSE: KeyNumType = VK_RBUTTON as KeyNumType;
pub const KN_RSHIFT: KeyNumType = VK_SHIFT as KeyNumType;
pub const KN_SCROLLLOCK: KeyNumType = VK_SCROLL as KeyNumType;
pub const KN_SEMICOLON: KeyNumType = VK_NONE_BA as KeyNumType;
pub const KN_SLASH: KeyNumType = VK_NONE_BF as KeyNumType;
pub const KN_SPACE: KeyNumType = VK_SPACE as KeyNumType;
pub const KN_SQUOTE: KeyNumType = VK_NONE_DE as KeyNumType;
pub const KN_TAB: KeyNumType = VK_TAB as KeyNumType;
pub const KN_UP: KeyNumType = VK_UP as KeyNumType;
pub const KN_UPLEFT: KeyNumType = VK_HOME as KeyNumType;
pub const KN_UPRIGHT: KeyNumType = VK_PRIOR as KeyNumType;
pub const KN_SHIFT_BIT: KeyNumType = WWKEY_SHIFT_BIT;
pub const KN_CTRL_BIT: KeyNumType = WWKEY_CTRL_BIT;
pub const KN_ALT_BIT: KeyNumType = WWKEY_ALT_BIT;
pub const KN_RLSE_BIT: KeyNumType = WWKEY_RLS_BIT;
pub const KN_BUTTON: KeyNumType = WWKEY_BTN_BIT;
