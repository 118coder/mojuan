//! 版本更新：检查 GitHub Releases、下载新 exe、重启替换。
//!
//! 交互设计：
//! - 启动后 2 秒静默检查一次（可在首选项里关掉）
//! - 「更多」菜单里可手动检查
//! - 有新版本时弹窗，说明变更 + 一键下载更新（下载完自动替换并重启）

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};

use mojuan_core::update::{self, Release};

/// 默认更新源；留空表示"未配置"（首选项里可以手填 owner/repo）
pub const DEFAULT_REPO: &str = "";

pub enum Msg {
    Checked(Result<Release, String>),
    Downloaded(Result<PathBuf, String>),
}

#[derive(Default)]
pub enum Status {
    #[default]
    Idle,
    Checking,
    /// 已经是最新
    Latest(String),
    /// 发现新版本
    Found(Release),
    Downloading,
    /// 下载完成，等待重启替换
    Ready(PathBuf),
    Failed(String),
}

pub struct Updater {
    pub status: Status,
    pub dialog_open: bool,
    pub auto_done: bool,
    pub last_check: Option<f64>,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
}

impl Default for Updater {
    fn default() -> Self {
        let (tx, rx) = channel();
        Self {
            status: Status::Idle,
            dialog_open: false,
            auto_done: false,
            last_check: None,
            tx,
            rx,
        }
    }
}

impl Updater {
    pub fn repo(&self, configured: &str) -> String {
        let from_env = std::env::var("MOJUAN_REPO").unwrap_or_default();
        if !configured.trim().is_empty() {
            configured.trim().to_string()
        } else if !from_env.trim().is_empty() {
            from_env
        } else {
            DEFAULT_REPO.to_string()
        }
    }

    pub fn busy(&self) -> bool {
        matches!(
            self.status,
            Status::Checking | Status::Downloading
        )
    }

    pub fn check(&mut self, repo: String, now: f64) {
        if self.busy() {
            return;
        }
        if repo.trim().is_empty() {
            self.status = Status::Failed("还没有配置更新源（首选项 → 更新源，填 owner/repo）".into());
            return;
        }
        self.status = Status::Checking;
        self.last_check = Some(now);
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let res = update::fetch_latest(&repo);
            let _ = tx.send(Msg::Checked(res));
        });
    }

    pub fn download(&mut self, release: &Release) -> Result<(), String> {
        let asset = release
            .exe_asset()
            .ok_or_else(|| "这个版本没有可下载的 exe".to_string())?;
        let url = asset.browser_download_url.clone();
        let dest = new_exe_path();
        self.status = Status::Downloading;
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let res = update::download(&url, &dest, 64 * 1024 * 1024).map(|_| dest);
            let _ = tx.send(Msg::Downloaded(res));
        });
        Ok(())
    }

    pub fn poll(&mut self, now: f64, current: &str, auto: bool) -> Vec<String> {
        let mut notes = Vec::new();
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Msg::Checked(Ok(release)) => {
                    let remote = release.version();
                    if update::is_newer(&remote, current) {
                        self.dialog_open = true;
                        self.status = Status::Found(release);
                    } else {
                        self.status = Status::Latest(remote);
                        if !auto {
                            notes.push("已是最新版本".to_string());
                        }
                    }
                }
                Msg::Checked(Err(e)) => {
                    self.status = Status::Failed(e.clone());
                    if !auto {
                        notes.push(e);
                    }
                }
                Msg::Downloaded(Ok(path)) => {
                    self.status = Status::Ready(path);
                }
                Msg::Downloaded(Err(e)) => {
                    self.status = Status::Failed(e.clone());
                    notes.push(e);
                }
            }
        }
        let _ = now;
        notes
    }

    /// 启动时自动检查（只做一次，延迟由调用方控制）
    pub fn maybe_auto(&mut self, enabled: bool, repo: &str, now: f64) {
        if !enabled || self.auto_done || now < 2.0 {
            return;
        }
        if repo.trim().is_empty() {
            self.auto_done = true;
            return;
        }
        self.auto_done = true;
        self.check(repo.to_string(), now);
    }
}

/// 新 exe 的落盘位置（与当前 exe 同目录，便于替换）
pub fn new_exe_path() -> PathBuf {
    let dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(std::env::temp_dir);
    dir.join("mojuan.update.exe")
}

/// 生成替换脚本并退出程序：等本进程退出 → 覆盖 → 启动新版本 → 自删
pub fn install_and_restart(new_exe: &std::path::Path) -> Result<(), String> {
    let cur = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = cur.parent().ok_or("找不到程序目录")?.to_path_buf();
    let bat = dir.join("mojuan-update.bat");
    let script = format!(
        "@echo off\r\n\
         chcp 65001 >nul\r\n\
         ping 127.0.0.1 -n 3 >nul\r\n\
         move /y \"{new}\" \"{cur}\" >nul\r\n\
         start \"\" \"{cur}\"\r\n\
         del \"%~f0\"\r\n",
        new = new_exe.display(),
        cur = cur.display()
    );
    std::fs::write(&bat, script).map_err(|e| format!("写入更新脚本失败：{e}"))?;

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED: u32 = 0x0000_0008;
        const NO_WINDOW: u32 = 0x0800_0000;
        std::process::Command::new("cmd")
            .args(["/c", "start", "", "/b"])
            .arg(&bat)
            .creation_flags(DETACHED | NO_WINDOW)
            .spawn()
            .map_err(|e| format!("启动更新脚本失败：{e}"))?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        return Err("当前平台不支持自动替换".into());
    }

    std::process::exit(0);
}
