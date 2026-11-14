//! 调试用：验证"原地替换 + 失败回滚"。
use std::path::Path;

fn main() {
    let dir = std::env::temp_dir().join("mojuan-swap-test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let cur = dir.join("mojuan.exe");
    let new = dir.join("mojuan.update.exe");
    std::fs::write(&cur, "OLD").unwrap();
    std::fs::write(&new, "NEWVERSION").unwrap();

    let old = mojuan_core::update::swap_in_place(&cur, &new).expect("swap");
    println!("cur = {:?}", std::fs::read_to_string(&cur).unwrap());
    println!("old = {:?} ({})", std::fs::read_to_string(&old).unwrap(), old.display());
    println!("new exists = {}", new.exists());

    // 再验证一次失败回滚：new 不存在
    let _ = std::fs::remove_file(dir.join("mojuan.update.exe"));
    let r = mojuan_core::update::swap_in_place(&cur, Path::new(&dir.join("mojuan.update.exe")));
    println!("missing-new => {:?}", r.err());
    println!("cur intact = {:?}", std::fs::read_to_string(&cur).unwrap());
}
