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
    const DWMWA_BORDER_COLOR: u32 = 34;
    /// DWMWA_COLOR_NONE：不画描边
    const DWMWA_COLOR_NONE: u32 = 0xFF_FF_FF_FE;

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

    /// COLORREF = 0x00BBGGRR
    pub fn set_border(hwnd: isize, dark: bool, fullscreen: bool) {
        let color: i32 = if fullscreen {
            DWMWA_COLOR_NONE as i32
        } else if dark {
            // #2C2C2C
            0x00_2C_2C_2C_u32 as i32
        } else {
            // #E1E4E8 → BB=0xE8 GG=0xE4 RR=0xE1
            0x00_E8_E4_E1_u32 as i32
        };
        unsafe {
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_BORDER_COLOR,
                &color as *const i32 as *const c_void,
                std::mem::size_of::<i32>() as u32,
            );
        }
    }

    /// 主屏物理像素尺寸（离屏测试窗口定位用）
    pub fn screen_size_px() -> (i32, i32) {
        #[link(name = "user32")]
        extern "system" {
            fn GetSystemMetrics(i: i32) -> i32;
        }
        unsafe { (GetSystemMetrics(0), GetSystemMetrics(1)) }
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

/// 无边框窗口的观感：DWM 描边（主题色，全屏时关闭）+ 圆角。
/// 无边框窗口没有系统阴影，用 1px 描边把窗口从桌面上衬出来。
#[cfg(target_os = "windows")]
pub fn set_chrome(frame: &eframe::Frame, dark: bool, fullscreen: bool) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    if let Ok(handle) = frame.window_handle() {
        if let RawWindowHandle::Win32(w) = handle.as_raw() {
            imp::set_border(w.hwnd.get(), dark, fullscreen);
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn set_chrome(_frame: &eframe::Frame, _dark: bool, _fullscreen: bool) {}

/// 主屏物理像素尺寸（离屏测试窗口定位用）
#[cfg(target_os = "windows")]
pub fn screen_size_px() -> (i32, i32) {
    imp::screen_size_px()
}

#[cfg(not(target_os = "windows"))]
pub fn screen_size_px() -> (i32, i32) {
    (1920, 1080)
}


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


/// exe 自身的构建时刻（文件最后写入时间 → 本地 "MM-dd HH:mm"）。
/// 窗口标题显示它，用于肉眼区分新旧 exe（右键彻查经验）。
pub fn exe_build_time_str() -> Option<String> {
    #[repr(C)]
    struct FileTime(u64);
    #[link(name = "kernel32")]
    extern "system" {
        fn CreateFileW(
            name: *const u16,
            access: u32,
            share: u32,
            security: *const u8,
            disposition: u32,
            flags: u32,
            template: isize,
        ) -> isize;
        fn CloseHandle(h: isize) -> i32;
        fn GetFileTime(h: isize, created: *mut u64, accessed: *mut u64, written: *mut FileTime) -> i32;
        fn FileTimeToSystemTime(ft: *const FileTime, st: *mut SYSTEMTIME) -> i32;
        fn SystemTimeToTzSpecificLocalTime(tz: *const u8, utc: *const SYSTEMTIME, local: *mut SYSTEMTIME) -> i32;
    }
    let exe = std::env::current_exe().ok()?;
    use std::os::windows::ffi::OsStrExt;
    let wide: Vec<u16> = exe.as_os_str().encode_wide().chain([0]).collect();
    unsafe {
        // GENERIC_READ(0x80000000) FILE_SHARE_READ|WRITE|DELETE(7) OPEN_EXISTING(3)
        let h = CreateFileW(wide.as_ptr(), 0x8000_0000, 7, std::ptr::null(), 3, 0, 0);
        if h == -1 {
            return None;
        }
        let mut ft = FileTime(0);
        let ok = GetFileTime(h, std::ptr::null_mut(), std::ptr::null_mut(), &mut ft);
        CloseHandle(h);
        if ok == 0 {
            return None;
        }
        let mut utc = SYSTEMTIME {
            year: 0, month: 0, day_of_week: 0, day: 0,
            hour: 0, minute: 0, second: 0, millis: 0,
        };
        if FileTimeToSystemTime(&ft, &mut utc) == 0 {
            return None;
        }
        let mut local = SYSTEMTIME {
            year: 0, month: 0, day_of_week: 0, day: 0,
            hour: 0, minute: 0, second: 0, millis: 0,
        };
        if SystemTimeToTzSpecificLocalTime(std::ptr::null(), &utc, &mut local) == 0 {
            local = utc;
        }
        Some(format!("{:02}-{:02} {:02}:{:02}", local.month, local.day, local.hour, local.minute))
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
