//! 版本检查与下载：通过 GitHub Releases 判断是否有新版本。

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::http;

#[derive(Debug, Clone, Deserialize)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
    #[serde(default)]
    pub size: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Release {
    pub tag_name: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub html_url: String,
    #[serde(default)]
    pub assets: Vec<Asset>,
}

impl Release {
    /// 只认 exe 资源；没有就认为该版本不可自动更新
    pub fn exe_asset(&self) -> Option<&Asset> {
        self.assets
            .iter()
            .find(|a| a.name.to_ascii_lowercase().ends_with(".exe"))
    }

    pub fn version(&self) -> String {
        // tag 可能是 jq-1.8.2 / v1.2.3 / release-2024.05 之类，能解析就统一成 x.y.z
        match Version::parse(&self.tag_name) {
            Some(v) => v.to_string(),
            None => self.tag_name.trim_start_matches(['v', 'V']).to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    /// 从任意形式的 tag 里取出第一段 x.y.z。
    ///
    /// 兼容 `1.2.3`、`v1.2.3`、`jq-1.8.2`、`release-2024.05.1`、`1.2.3-beta.1`
    pub fn parse(s: &str) -> Option<Self> {
        let chars: Vec<char> = s.trim().chars().collect();
        // 从第一个数字开始
        let start = chars.iter().position(|c| c.is_ascii_digit())?;
        let mut end = start;
        while end < chars.len() && (chars[end].is_ascii_digit() || chars[end] == '.') {
            end += 1;
        }
        let core: String = chars[start..end].iter().collect();
        let core = core.trim_matches('.');
        let mut it = core.split('.');
        let major = it.next()?.parse().ok()?;
        let minor = it.next().unwrap_or("0").parse().ok()?;
        let patch = it.next().unwrap_or("0").parse().ok()?;
        Some(Self {
            major,
            minor,
            patch,
        })
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// 拉取最新 release。`repo` 形如 `owner/name`。
pub fn fetch_latest(repo: &str) -> Result<Release, String> {
    let repo = repo.trim().trim_matches('/');
    if repo.split('/').count() != 2 {
        return Err("更新源格式应为 owner/repo".into());
    }
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let resp = http::https_get(&url, 512 * 1024, 8000)?;
    match resp.status {
        200 => serde_json::from_slice::<Release>(&resp.body)
            .map_err(|e| format!("解析版本信息失败：{e}")),
        404 => Err("仓库或 Release 不存在".into()),
        403 => Err("请求被 GitHub 限流，请稍后再试".into()),
        other => Err(format!("服务器返回 {other}")),
    }
}

/// 当前版本是否比远端旧
pub fn is_newer(remote: &str, current: &str) -> bool {
    match (Version::parse(remote), Version::parse(current)) {
        (Some(r), Some(c)) => r > c,
        _ => false,
    }
}

/// 下载到目标文件
pub fn download(url: &str, dest: &Path, max_bytes: usize) -> Result<u64, String> {
    let resp = http::https_get(url, max_bytes, 12000)?;
    if resp.status != 200 {
        return Err(format!("下载失败，服务器返回 {}", resp.status));
    }
    if resp.body.len() < 1024 {
        return Err("下载内容异常（可能被拦截）".into());
    }
    if let Some(dir) = dest.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    std::fs::write(dest, &resp.body).map_err(|e| format!("写入文件失败：{e}"))?;
    Ok(resp.body.len() as u64)
}

/// 就地把 `current` 换成 `new_exe`，旧文件保留为 `current` 同目录的 `*.old`。
///
/// Windows 允许重命名正在运行的 exe，但不能覆盖它，所以顺序是
/// 先把旧文件改名让位，再让新文件顶上；任何一步失败都回滚，
/// 保证"程序还在"比"更新成功"更重要。
///
/// 返回旧文件的路径（调用方负责在新进程启动后删除）。
pub fn swap_in_place(current: &Path, new_exe: &Path) -> Result<PathBuf, String> {
    if !new_exe.exists() {
        return Err("更新文件已不存在，请重新下载".into());
    }
    let dir = current.parent().ok_or("找不到程序目录")?;
    let stem = current
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "mojuan".into());
    let ext = current
        .extension()
        .map(|s| format!(".{}", s.to_string_lossy()))
        .unwrap_or_default();
    let old = dir.join(format!("{stem}.old{ext}"));

    if old.exists() {
        let _ = std::fs::remove_file(&old);
    }

    std::fs::rename(current, &old)
        .map_err(|e| format!("无法重命名当前程序（可能放在只读目录）：{e}"))?;

    if let Err(e) = std::fs::rename(new_exe, current) {
        let _ = std::fs::rename(&old, current);
        return Err(format!("替换失败：{e}"));
    }
    Ok(old)
}
