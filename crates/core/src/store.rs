//! 本地持久化：设置、最近打开、工作区历史。
//! 优先存在程序目录（便携），不可写时回退到系统配置目录。

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ViewMode {
    Read,
    #[default]
    Split,
    Edit,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    pub theme: Theme,
    pub mode: ViewMode,
    /// 正文字号
    pub font_size: f32,
    /// 阅读栏宽（正文字列宽）
    pub measure: f32,
    /// 行距倍数
    pub line_spacing: f32,
    pub sidebar_width: f32,
    pub sidebar_open: bool,
    pub autosave: bool,
    /// 分屏时编辑区占比
    pub split_ratio: f32,
    /// 启动时自动检查更新
    pub check_updates: bool,
    /// 更新源（GitHub owner/repo），留空则用内置默认值
    pub update_repo: String,
    pub last_workspace: Option<String>,
    pub last_file: Option<String>,
    /// 窗口位置与尺寸 [x, y, w, h]
    pub window: Option<[f32; 4]>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::Dark,
            mode: ViewMode::Split,
            font_size: 16.0,
            measure: 720.0,
            line_spacing: 1.75,
            sidebar_width: 268.0,
            sidebar_open: true,
            autosave: false,
            split_ratio: 0.5,
            check_updates: true,
            update_repo: String::new(),
            last_workspace: None,
            last_file: None,
            window: None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RecentFile {
    pub path: String,
    pub ts: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Store {
    pub settings: Settings,
    pub recent: Vec<RecentFile>,
    pub workspaces: Vec<String>,
}

pub fn now_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn data_dir() -> PathBuf {
    // 便携优先：程序同目录 data/
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let portable = dir.join("data");
            if std::fs::create_dir_all(&portable).is_ok() {
                let probe = portable.join(".writable");
                if std::fs::write(&probe, b"1").is_ok() {
                    let _ = std::fs::remove_file(&probe);
                    return portable;
                }
            }
        }
    }
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join("Mojuan");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

impl Store {
    pub fn state_path() -> PathBuf {
        data_dir().join("state.json")
    }

    pub fn load() -> Self {
        let path = Self::state_path();
        match std::fs::read_to_string(&path) {
            Ok(s) => serde_json::from_str(s.trim_start_matches('\u{feff}')).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::state_path();
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&path, json).map_err(|e| format!("保存设置失败：{e}"))
    }

    /// 记录最近打开的文件（去重、置顶、最多 40 条）
    pub fn touch_file(&mut self, path: &Path) {
        let p = path.to_string_lossy().to_string();
        self.recent.retain(|r| r.path != p);
        self.recent.insert(
            0,
            RecentFile {
                path: p,
                ts: now_ts(),
            },
        );
        self.recent.truncate(40);
    }

    pub fn forget_file(&mut self, path: &Path) {
        let p = path.to_string_lossy().to_string();
        self.recent.retain(|r| r.path != p);
    }

    pub fn touch_workspace(&mut self, path: &Path) {
        let p = path.to_string_lossy().to_string();
        self.workspaces.retain(|w| w != &p);
        self.workspaces.insert(0, p.clone());
        self.workspaces.truncate(12);
        self.settings.last_workspace = Some(p);
    }
}
