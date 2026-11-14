//! 调试用：按仓库名下载最新的 exe 附件，验证 WinHTTP 下载链路。
use std::path::Path;

fn main() {
    let repo = std::env::args().nth(1).unwrap_or_else(|| "jqlang/jq".into());
    let rel = mojuan_core::update::fetch_latest(&repo).expect("fetch");
    let asset = rel.exe_asset().expect("no exe asset");
    println!("asset={} size={}", asset.name, asset.size);
    let dest = std::env::temp_dir().join("mojuan-dl-test.exe");
    let t0 = std::time::Instant::now();
    let n = mojuan_core::update::download(&asset.browser_download_url, Path::new(&dest), 64 * 1024 * 1024)
        .expect("download");
    let head = std::fs::read(&dest).unwrap_or_default();
    println!(
        "downloaded {n} bytes in {:.2}s -> {} (magic={:?})",
        t0.elapsed().as_secs_f32(),
        dest.display(),
        head.get(..2)
    );
}
