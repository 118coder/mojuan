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
pub const DEFAULT_REPO: &str = "118coder/mojuan";

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
    /// 右下角是否有"有新版本"的小浮窗
    pub banner: bool,
    pub auto_done: bool,
    /// 本次检查是否由启动自动触发（失败时不打扰用户）
    pub auto_pending: bool,
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
            banner: false,
            auto_done: false,
            auto_pending: false,
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

    pub fn poll(&mut self, now: f64, current: &str) -> Vec<String> {
        let auto = self.auto_pending;
        let mut notes = Vec::new();
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Msg::Checked(Ok(release)) => {
                    self.auto_pending = false;
                    let remote = release.version();
                    if update::is_newer(&remote, current) {
                        self.status = Status::Found(release);
                        // 只在用户主动检查时才弹窗；自动检查只亮起右下角的小浮窗
                        if !auto {
                            self.dialog_open = true;
                        } else {
                            self.banner = true;
                        }
                    } else {
                        self.status = Status::Latest(remote);
                        if !auto {
                            notes.push("已是最新版本".to_string());
                        }
                    }
                }
                Msg::Checked(Err(e)) => {
                    self.auto_pending = false;
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
        // 启动后延迟一点，避免和首屏渲染抢资源；测试时可用 MJ_AUTO_WAIT 覆盖
        let wait: f64 = std::env::var("MJ_AUTO_WAIT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(2.0);
        if !enabled || self.auto_done || now < wait {
            return;
        }
        if repo.trim().is_empty() {
            self.auto_done = true;
            return;
        }
        self.auto_done = true;
        self.auto_pending = true;
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

/// 就地把当前 exe 换成新版本，然后启动新进程并退出。
///
/// 交换文件由 `mojuan_core::update::swap_in_place` 完成（失败会回滚），
/// 这里只负责"拉起新版本 → 退出 → 下次启动清理旧文件"。
pub fn install_and_restart(new_exe: &std::path::Path) -> Result<(), String> {
    let cur = std::env::current_exe().map_err(|e| e.to_string())?;
    update::swap_in_place(&cur, new_exe)?;

    let mut cmd = std::process::Command::new(&cur);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED: u32 = 0x0000_0008;
        cmd.creation_flags(DETACHED);
    }
    cmd.spawn()
        .map_err(|e| format!("新版本启动失败：{e}。请手动运行 {cur:?}"))?;

    std::process::exit(0);
}

/// 启动时清理上一次更新留下的旧 exe（失败也无所谓）
pub fn cleanup_old_exe() {
    if let Ok(cur) = std::env::current_exe() {
        if let Some(dir) = cur.parent() {
            let old = dir.join("mojuan.old.exe");
            if old.exists() {
                let _ = std::fs::remove_file(&old);
            }
        }
    }
}
