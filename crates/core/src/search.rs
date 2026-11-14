//! 工作区搜索：文件名匹配 + 全文检索 + 模糊匹配打分（给快速打开用）。

use std::path::{Path, PathBuf};

use crate::files;

#[derive(Debug, Clone)]
pub struct NameHit {
    pub path: PathBuf,
    pub name: String,
    pub rel: String,
}

#[derive(Debug, Clone)]
pub struct ContentHit {
    pub path: PathBuf,
    pub name: String,
    pub line_no: usize,
    /// 命中行原文（已裁剪长度）
    pub line: String,
    /// 命中位置（字符索引，用于高亮）
    pub start: usize,
    pub len: usize,
}

#[derive(Debug, Clone, Default)]
pub struct SearchResult {
    pub names: Vec<NameHit>,
    pub contents: Vec<ContentHit>,
    pub truncated: bool,
}

const MAX_FILES: usize = 4000;
const MAX_CONTENT_HITS: usize = 200;
const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;

pub fn search(root: &Path, query: &str) -> SearchResult {
    let q = query.trim();
    let mut result = SearchResult::default();
    if q.is_empty() {
        return result;
    }
    let needle = q.to_lowercase();

    for path in files::walk_md(root, MAX_FILES) {
        let name = files::display_name(&path);
        let rel = files::rel_display(root, &path);
        if name.to_lowercase().contains(&needle) || rel.to_lowercase().contains(&needle) {
            result.names.push(NameHit {
                name: name.clone(),
                rel: rel.clone(),
                path: path.clone(),
            });
        }
    }

    if q.chars().count() >= 2 {
        'outer: for path in files::walk_md(root, MAX_FILES) {
            let meta = match std::fs::metadata(&path) {
                Ok(m) => m,
                Err(_) => continue,
            };
            if meta.len() > MAX_FILE_BYTES {
                continue;
            }
            let Ok((text, _)) = files::read_doc(&path) else {
                continue;
            };
            let name = files::display_name(&path);
            for (i, line) in text.lines().enumerate() {
                let lower = line.to_lowercase();
                if let Some(byte_pos) = lower.find(&needle) {
                    let start = lower[..byte_pos].chars().count();
                    let snippet = snippet(line, start, q.chars().count(), 90);
                    result.contents.push(ContentHit {
                        path: path.clone(),
                        name: name.clone(),
                        line_no: i + 1,
                        line: snippet.0,
                        start: snippet.1,
                        len: snippet.2,
                    });
                    if result.contents.len() >= MAX_CONTENT_HITS {
                        result.truncated = true;
                        break 'outer;
                    }
                }
            }
        }
    }

    result.names.truncate(200);
    result
}

/// 截取命中行前后的上下文，返回 (片段, 命中在片段中的字符位置, 长度)
fn snippet(line: &str, start: usize, len: usize, max_chars: usize) -> (String, usize, usize) {
    let chars: Vec<char> = line.chars().collect();
    if chars.len() <= max_chars {
        return (line.to_string(), start, len);
    }
    let pad = max_chars / 3;
    let from = start.saturating_sub(pad);
    let to = (from + max_chars).min(chars.len());
    let mut s: String = chars[from..to].iter().collect();
    if from > 0 {
        s.insert(0, '…');
    }
    if to < chars.len() {
        s.push('…');
    }
    let offset = start - from + usize::from(from > 0);
    (s, offset, len)
}

/// 模糊匹配打分：越大越匹配，None 表示不匹配
pub fn fuzzy_score(query: &str, target: &str) -> Option<i32> {
    let q: Vec<char> = query.chars().flat_map(|c| c.to_lowercase()).collect();
    if q.is_empty() {
        return Some(0);
    }
    let t: Vec<char> = target.chars().flat_map(|c| c.to_lowercase()).collect();
    let mut score = 0i32;
    let mut ti = 0usize;
    let mut streak = 0i32;
    for &qc in &q {
        let mut found = false;
        while ti < t.len() {
            if t[ti] == qc {
                score += 8 + streak * 4;
                if ti == 0 || !t[ti - 1].is_alphanumeric() {
                    score += 6; // 词首加成
                }
                streak += 1;
                ti += 1;
                found = true;
                break;
            }
            streak = 0;
            ti += 1;
            score -= 1;
        }
        if !found {
            return None;
        }
    }
    // 越短的目标越优先
    score -= (t.len() as i32) / 4;
    Some(score)
}
