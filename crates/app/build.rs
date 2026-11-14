//! 把图标和版本信息编译进 exe（Windows 资源）。
//! 没有资源编译器时不会让构建失败，只是 exe 没有文件图标。

fn main() {
    println!("cargo:rerun-if-changed=assets/mojuan.ico");
    println!("cargo:rerun-if-changed=build.rs");

    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/mojuan.ico");
        res.set("ProductName", "Mojuan 墨卷");
        res.set("FileDescription", "墨卷 · Markdown 阅读与编辑器");
        res.set("LegalCopyright", "MIT License");
        res.set("OriginalFilename", "mojuan.exe");
        res.set(
            "FileVersion",
            &format!("{}.{}.0.0", env!("CARGO_PKG_VERSION_MAJOR"), env!("CARGO_PKG_VERSION_MINOR")),
        );
        res.set("ProductVersion", env!("CARGO_PKG_VERSION"));
        if let Err(e) = res.compile() {
            println!("cargo:warning=嵌入 Windows 资源失败（exe 将没有文件图标）：{e}");
        }
    }
}
