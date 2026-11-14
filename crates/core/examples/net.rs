//! 调试用：测试 HTTPS 请求（默认打 GitHub API）。
fn main() {
    let url = std::env::args().nth(1).unwrap_or_else(|| "https://api.github.com/zen".into());
    match mojuan_core::http::https_get(&url, 256 * 1024, 8000) {
        Ok(r) => println!("status={} bytes={}\n{}", r.status, r.body.len(), r.text()),
        Err(e) => println!("ERROR: {e}"),
    }
}
