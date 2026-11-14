//! 版本检查与下载：通过 GitHub Releases 判断是否有新版本。

use std::path::Path;

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
        self.tag_name.trim_start_matches(['v', 'V']).to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    pub fn parse(s: &str) -> Option<Self> {
        let core = s.trim().trim_start_matches(['v', 'V']);
        let core = core.split(['-', '+']).next().unwrap_or(core);
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
