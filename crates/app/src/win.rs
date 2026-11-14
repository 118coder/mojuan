//! Windows 相关的小修补：让系统标题栏跟随暗色主题。
//!
//! 不引入 windows-rs，直接用 extern 声明 dwmapi，
//! 这样对体积和编译时间几乎没有影响。

#[cfg(target_os = "windows")]
mod imp {
    use std::ffi::c_void;

    #[link(name = "dwmapi")]
    extern "system" {
        fn DwmSetWindowAttribute(
            hwnd: isize,
            attribute: u32,
            value: *const c_void,
            size: u32,
        ) -> i32;
    }

    const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;
    const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;

    pub fn set_dark_titlebar(hwnd: isize, dark: bool) {
        let v: i32 = if dark { 1 } else { 0 };
        unsafe {
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &v as *const i32 as *const c_void,
                std::mem::size_of::<i32>() as u32,
            );
        }
        // 圆角：2 = 圆角
        let corner: i32 = 2;
        unsafe {
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                &corner as *const i32 as *const c_void,
                std::mem::size_of::<i32>() as u32,
            );
        }
    }
}

#[cfg(target_os = "windows")]
pub fn apply(frame: &eframe::Frame, dark: bool) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    if let Ok(handle) = frame.window_handle() {
        if let RawWindowHandle::Win32(w) = handle.as_raw() {
            imp::set_dark_titlebar(w.hwnd.get(), dark);
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn apply(_frame: &eframe::Frame, _dark: bool) {}
