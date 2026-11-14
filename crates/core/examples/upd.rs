//! 调试用：检查某个仓库的最新 release 与版本比较。
fn main() {
    let repo = std::env::args().nth(1).unwrap_or_else(|| "BurntSushi/ripgrep".into());
    match mojuan_core::update::fetch_latest(&repo) {
        Ok(r) => {
            println!("tag={} version={} assets={}", r.tag_name, r.version(), r.assets.len());
            for a in &r.assets {
                println!("  - {} ({} bytes)", a.name, a.size);
            }
            println!("exe asset = {:?}", r.exe_asset().map(|a| a.name.clone()));
            println!("is_newer(0.1.0) = {}", mojuan_core::update::is_newer(&r.version(), "0.1.0"));
        }
        Err(e) => println!("ERROR: {e}"),
    }
}
