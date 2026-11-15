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


/// 本地时间 "YYYY-MM-DD HH:MM"（Ctrl+T 插入时间戳用）
#[repr(C)]
struct SYSTEMTIME {
    year: u16,
    month: u16,
    day_of_week: u16,
    day: u16,
    hour: u16,
    minute: u16,
    second: u16,
    millis: u16,
}

pub fn local_time_str() -> String {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetLocalTime(t: *mut SYSTEMTIME);
    }
    unsafe {
        let mut st = SYSTEMTIME {
            year: 0, month: 0, day_of_week: 0, day: 0,
            hour: 0, minute: 0, second: 0, millis: 0,
        };
        GetLocalTime(&mut st);
        format!(
            "{}-{:02}-{:02} {:02}:{:02}",
            st.year, st.month, st.day, st.hour, st.minute
        )
    }
}


/// UTF-8 → GBK 字节（ANSI 另存用）。含无法映射字符时返回 None。
pub fn utf8_to_gbk(s: &str) -> Option<Vec<u8>> {
    #[link(name = "kernel32")]
    extern "system" {
        fn WideCharToMultiByte(
            code_page: u32, flags: u32, src: *const u16, src_len: i32,
            dst: *mut u8, dst_len: i32, default_char: *const u8, used_default: *mut u8,
        ) -> i32;
    }
    const CP_GBK: u32 = 936;
    let wide: Vec<u16> = s.encode_utf16().collect();
    unsafe {
        let n = WideCharToMultiByte(
            CP_GBK, 0, wide.as_ptr(), wide.len() as i32,
            std::ptr::null_mut(), 0, std::ptr::null(), std::ptr::null_mut(),
        );
        if n <= 0 {
            return None;
        }
        let mut buf = vec![0u8; n as usize];
        let mut used_default = 0u8;
        WideCharToMultiByte(
            CP_GBK, 0, wide.as_ptr(), wide.len() as i32,
            buf.as_mut_ptr(), n, std::ptr::null(), &mut used_default,
        );
        if used_default != 0 {
            return None; // 有字符无法映射成 GBK
        }
        Some(buf)
    }
}
