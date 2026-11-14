//! 文件系统相关：目录浏览、读写（含 GBK 兼容）、新建 / 重命名 / 删除（进回收站）。

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub const MD_EXTS: &[&str] = &["md", "markdown", "mdown", "mkd", "mdx", "txt"];

const IGNORED_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    ".obsidian",
    ".trash",
    ".idea",
    ".vscode",
    "__pycache__",
    ".next",
    ".cache",
];

#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
}

pub fn is_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| MD_EXTS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

pub fn ext_of(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

fn ignored(dir_name: &str) -> bool {
    dir_name.starts_with('.') || IGNORED_DIRS.iter().any(|d| d.eq_ignore_ascii_case(dir_name))
}

/// 列出一层目录：目录在前，其次按名称排序；只保留目录与 Markdown/文本文件
pub fn list_dir(dir: &Path) -> Vec<Entry> {
    let mut dirs: Vec<Entry> = Vec::new();
    let mut files: Vec<Entry> = Vec::new();

    let Ok(rd) = fs::read_dir(dir) else {
        return Vec::new();
    };
    for item in rd.flatten() {
        let path = item.path();
        let name = item.file_name().to_string_lossy().to_string();
        let is_dir = item.file_type().map(|t| t.is_dir()).unwrap_or(false);
        if is_dir {
            if ignored(&name) {
                continue;
            }
            dirs.push(Entry {
                name,
                path,
                is_dir: true,
            });
        } else {
            if name.starts_with('.') || !is_markdown(&path) {
                continue;
            }
            files.push(Entry {
                name,
                path,
                is_dir: false,
            });
        }
    }

    let key = |e: &Entry| e.name.to_lowercase();
    dirs.sort_by_key(key);
    files.sort_by_key(key);
    dirs.extend(files);
    dirs
}

/// 递归收集 Markdown 文件（带数量上限，避免大仓库卡住）
pub fn walk_md(root: &Path, limit: usize) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if out.len() >= limit {
            break;
        }
        let Ok(rd) = fs::read_dir(&dir) else { continue };
        let mut sub = Vec::new();
        for item in rd.flatten() {
            let path = item.path();
            let name = item.file_name().to_string_lossy().to_string();
            let is_dir = item.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if is_dir {
                if !ignored(&name) {
                    sub.push(path);
                }
            } else if !name.starts_with('.') && is_markdown(&path) {
                out.push(path);
            }
        }
        // 保持目录顺序稳定
        sub.sort();
        stack.extend(sub.into_iter().rev());
    }
    out.sort_by_key(|p| p.to_string_lossy().to_lowercase());
    out
}

/// 读取文本：优先 UTF-8，失败时按 GB18030 解码（兼容老的中文文件）。
/// 返回 (内容, 是否为 CRLF 换行)，内容统一归一化成 \n。
pub fn read_doc(path: &Path) -> Result<(String, bool), String> {
    let bytes = fs::read(path).map_err(|e| format!("读取失败：{e}"))?;
    if bytes.len() > 40 * 1024 * 1024 {
        return Err("文件过大（超过 40MB）".into());
    }
    let (decoded, _, _) = encoding_rs::UTF_8.decode(&bytes);
    let mut text = if decoded.contains('\u{FFFD}') {
        let (gbk, _, _) = encoding_rs::GB18030.decode(&bytes);
        gbk.into_owned()
    } else {
        decoded.into_owned()
    };
    if text.starts_with('\u{FEFF}') {
        text.remove(0);
    }
    let crlf = text.contains("\r\n");
    if crlf {
        text = text.replace("\r\n", "\n");
    }
    Ok((text, crlf))
}

pub fn write_doc(path: &Path, text: &str, crlf: bool) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| format!("创建目录失败：{e}"))?;
        }
    }
    let data = if crlf {
        text.replace('\n', "\r\n")
    } else {
        text.to_string()
    };
    fs::write(path, data.as_bytes()).map_err(|e| format!("保存失败：{e}"))
}

pub fn mtime(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).ok()?.modified().ok()
}

fn sanitize_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if (c as u32) < 32 => '_',
            c => c,
        })
        .collect();
    cleaned.trim().trim_matches('.').to_string()
}

fn unique_path(dir: &Path, stem: &str, ext: Option<&str>) -> PathBuf {
    let build = |n: usize| -> PathBuf {
        let name = match ext {
            Some(e) if !e.is_empty() => {
                if n == 0 {
                    format!("{stem}.{e}")
                } else {
                    format!("{stem} {n}.{e}")
                }
            }
            _ => {
                if n == 0 {
                    stem.to_string()
                } else {
                    format!("{stem} {n}")
                }
            }
        };
        dir.join(name)
    };
    let mut n = 0;
    loop {
        let p = build(n);
        if !p.exists() {
            return p;
        }
        n += 1;
        if n > 999 {
            return p;
        }
    }
}

pub fn create_file(dir: &Path, name: &str) -> Result<PathBuf, String> {
    let name = sanitize_name(name);
    if name.is_empty() {
        return Err("文件名不能为空".into());
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s.to_string(), Some(e.to_string())),
        _ => (name.clone(), Some("md".to_string())),
    };
    let path = unique_path(dir, &stem, ext.as_deref());
    write_doc(&path, "", false)?;
    Ok(path)
}

pub fn create_dir(dir: &Path, name: &str) -> Result<PathBuf, String> {
    let name = sanitize_name(name);
    if name.is_empty() {
        return Err("文件夹名不能为空".into());
    }
    let path = unique_path(dir, &name, None);
    fs::create_dir(&path).map_err(|e| format!("创建文件夹失败：{e}"))?;
    Ok(path)
}

pub fn rename(path: &Path, new_name: &str) -> Result<PathBuf, String> {
    let new_name = sanitize_name(new_name);
    if new_name.is_empty() {
        return Err("名称不能为空".into());
    }
    let parent = path.parent().ok_or("无法定位父目录")?;
    let mut target = parent.join(&new_name);
    if target == path {
        return Ok(path.to_path_buf());
    }
    if target.exists() {
        let ext = if path.is_dir() {
            None
        } else {
            path.extension().and_then(|e| e.to_str())
        };
        let stem = Path::new(&new_name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&new_name)
            .to_string();
        target = unique_path(parent, &stem, ext);
    }
    fs::rename(path, &target).map_err(|e| format!("重命名失败：{e}"))?;
    Ok(target)
}

/// 删除到回收站（绝不直接抹掉用户数据）
pub fn delete(path: &Path) -> Result<(), String> {
    trash::delete(path).map_err(|e| format!("移入回收站失败：{e}"))
}

/// 在资源管理器中定位
pub fn reveal(path: &Path) {
    #[cfg(target_os = "windows")]
    {
        let arg = if path.is_dir() {
            path.to_path_buf()
        } else {
            path.to_path_buf()
        };
        let _ = std::process::Command::new("explorer").arg(arg).spawn();
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Some(p) = path.parent() {
            let _ = std::process::Command::new("xdg-open").arg(p).spawn();
        }
    }
}

pub fn rel_display(root: &Path, path: &Path) -> String {
    match path.strip_prefix(root) {
        Ok(p) => p.to_string_lossy().replace('\\', "/"),
        Err(_) => path.to_string_lossy().to_string(),
    }
}

pub fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string())
}
