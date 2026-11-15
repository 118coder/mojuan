//! 极简 HTTPS GET：直接调 Windows 自带的 WinHTTP。
//!
//! 为什么不引 reqwest / ureq：为了检查一次更新多带 1-3MB 的 TLS 依赖不划算。
//! WinHTTP 在系统里，体积零成本。

use std::ffi::c_void;

#[derive(Debug, Clone)]
pub struct Response {
    pub status: u16,
    pub body: Vec<u8>,
}

impl Response {
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).to_string()
    }
}

/// 发起一次 GET 请求，带超时与最大长度保护
pub fn https_get(url: &str, max_bytes: usize, timeout_ms: u32) -> Result<Response, String> {
    #[cfg(target_os = "windows")]
    {
        win::get(url, max_bytes, timeout_ms)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (url, max_bytes, timeout_ms);
        Err("当前平台暂不支持联网检查更新".to_string())
    }
}

#[cfg(target_os = "windows")]
mod win {
    use super::{c_void, Response};
    use std::ptr::null;

    type Handle = *mut c_void;

    #[link(name = "winhttp")]
    extern "system" {
        fn WinHttpOpen(
            agent: *const u16,
            access_type: u32,
            proxy: *const u16,
            bypass: *const u16,
            flags: u32,
        ) -> Handle;
        fn WinHttpConnect(session: Handle, server: *const u16, port: u16, reserved: u32) -> Handle;
        fn WinHttpOpenRequest(
            connect: Handle,
            verb: *const u16,
            path: *const u16,
            version: *const u16,
            referrer: *const u16,
            accept_types: *const *const u16,
            flags: u32,
        ) -> Handle;
        fn WinHttpSetTimeouts(
            session: Handle,
            resolve: i32,
            connect: i32,
            send: i32,
            receive: i32,
        ) -> i32;
        fn WinHttpSetOption(handle: Handle, option: u32, buffer: *const c_void, len: u32) -> i32;
        fn WinHttpSendRequest(
            request: Handle,
            headers: *const u16,
            headers_len: u32,
            optional: *const c_void,
            optional_len: u32,
            total_len: u32,
            context: usize,
        ) -> i32;
        fn WinHttpReceiveResponse(request: Handle, reserved: *mut c_void) -> i32;
        fn WinHttpQueryHeaders(
            request: Handle,
            info_level: u32,
            name: *const u16,
            buffer: *mut c_void,
            buffer_len: *mut u32,
            index: *mut u32,
        ) -> i32;
        fn WinHttpQueryDataAvailable(request: Handle, len: *mut u32) -> i32;
        fn WinHttpReadData(request: Handle, buffer: *mut c_void, to_read: u32, read: *mut u32)
            -> i32;
        fn WinHttpCloseHandle(handle: Handle) -> i32;
    }

    const WINHTTP_FLAG_SECURE: u32 = 0x0080_0000;
    const WINHTTP_QUERY_STATUS_CODE: u32 = 19;
    const WINHTTP_QUERY_FLAG_NUMBER: u32 = 0x2000_0000;
    const WINHTTP_OPTION_REDIRECT_POLICY: u32 = 88;
    // 0 = NEVER, 1 = 禁止 https→http, 2 = 总是跟随。
    // GitHub 的 release 附件会 302 到 CDN，必须跟随。
    const REDIRECT_ALWAYS: u32 = 2;

    struct Owned(Handle);
    impl Drop for Owned {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    WinHttpCloseHandle(self.0);
                }
            }
        }
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn split_url(url: &str) -> Result<(String, String, u16, bool), String> {
        let (rest, secure) = if let Some(r) = url.strip_prefix("https://") {
            (r, true)
        } else if let Some(r) = url.strip_prefix("http://") {
            (r, false)
        } else {
            return Err("只支持 http/https 地址".to_string());
        };
        let (host_port, path) = match rest.find('/') {
            Some(i) => (&rest[..i], &rest[i..]),
            None => (rest, "/"),
        };
        let (host, port) = match host_port.rsplit_once(':') {
            Some((h, p)) => (h.to_string(), p.parse::<u16>().unwrap_or(if secure { 443 } else { 80 })),
            None => (host_port.to_string(), if secure { 443 } else { 80 }),
        };
        if host.is_empty() {
            return Err("地址缺少主机名".into());
        }
        Ok((host, path.to_string(), port, secure))
    }

    pub fn get(url: &str, max_bytes: usize, timeout_ms: u32) -> Result<Response, String> {
        let (host, path, port, secure) = split_url(url)?;

        let agent = wide("Mojuan/1.0 (Windows)");
        let session = Owned(unsafe {
            WinHttpOpen(agent.as_ptr(), 0, null(), null(), 0)
        });
        if session.0.is_null() {
            return Err("初始化网络会话失败".into());
        }
        let t = timeout_ms as i32;
        unsafe {
            WinHttpSetTimeouts(session.0, t, t, t, t * 2);
            let policy = REDIRECT_ALWAYS;
            WinHttpSetOption(
                session.0,
                WINHTTP_OPTION_REDIRECT_POLICY,
                &policy as *const u32 as *const c_void,
                4,
            );
        }

        let host_w = wide(&host);
        let connect = Owned(unsafe { WinHttpConnect(session.0, host_w.as_ptr(), port, 0) });
        if connect.0.is_null() {
            return Err(format!("无法连接 {host}"));
        }

        let verb = wide("GET");
        let path_w = wide(&path);
        let request = Owned(unsafe {
            WinHttpOpenRequest(
                connect.0,
                verb.as_ptr(),
                path_w.as_ptr(),
                null(),
                null(),
                null(),
                if secure { WINHTTP_FLAG_SECURE } else { 0 },
            )
        });
        if request.0.is_null() {
            return Err("创建请求失败".into());
        }

        let headers = wide(
            "User-Agent: Mojuan-App\r\nAccept: application/vnd.github+json, */*\r\nCache-Control: no-cache\r\n",
        );
        let ok = unsafe {
            WinHttpSendRequest(
                request.0,
                headers.as_ptr(),
                u32::MAX,
                null(),
                0,
                0,
                0,
            )
        };
        if ok == 0 {
            return Err("发送请求失败（可能是网络不通）".into());
        }
        if unsafe { WinHttpReceiveResponse(request.0, null::<c_void>() as *mut c_void) } == 0 {
            return Err("等待响应失败".into());
        }

        // 状态码
        let mut status: u32 = 0;
        let mut len = 4u32;
        unsafe {
            WinHttpQueryHeaders(
                request.0,
                WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                null(),
                &mut status as *mut u32 as *mut c_void,
                &mut len,
                null::<u32>() as *mut u32,
            );
        }

        // 读 body
        let mut body: Vec<u8> = Vec::new();
        loop {
            let mut avail: u32 = 0;
            if unsafe { WinHttpQueryDataAvailable(request.0, &mut avail) } == 0 {
                break;
            }
            if avail == 0 {
                break;
            }
            let mut chunk = vec![0u8; avail.min(64 * 1024) as usize];
            let mut read: u32 = 0;
            if unsafe {
                WinHttpReadData(
                    request.0,
                    chunk.as_mut_ptr() as *mut c_void,
                    chunk.len() as u32,
                    &mut read,
                )
            } == 0
            {
                break;
            }
            if read == 0 {
                break;
            }
            chunk.truncate(read as usize);
            body.extend_from_slice(&chunk);
            if body.len() >= max_bytes {
                body.truncate(max_bytes);
                break;
            }
        }

        Ok(Response {
            status: status as u16,
            body,
        })
    }
}
