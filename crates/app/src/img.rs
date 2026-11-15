//! 图片加载：自己解码 + 纹理缓存。
//!
//! 不走 egui_extras 的异步 loader，原因：它的错误会被静默吞掉，
//! 而且要多带 ehttp / mime_guess 等依赖。这里用 image crate 解码，
//! 在后台线程里做，主线程只负责上传纹理。

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};

type Loaded = Result<Arc<ColorImage>, String>;

#[derive(Default)]
pub struct Loader {
    results: Arc<Mutex<HashMap<PathBuf, Loaded>>>,
    requested: HashSet<PathBuf>,
    textures: HashMap<PathBuf, TextureHandle>,
    failed: HashMap<PathBuf, String>,

    // 远程图片：键是 URL 字符串
    url_results: Arc<Mutex<HashMap<String, Loaded>>>,
    url_requested: HashSet<String>,
    url_textures: HashMap<String, TextureHandle>,
    url_failed: HashMap<String, String>,
}

impl Loader {
    /// 取得纹理；没有就发起后台加载，返回 None
    pub fn get(&mut self, ctx: &egui::Context, path: &Path) -> Option<&TextureHandle> {
        if self.textures.contains_key(path) {
            return self.textures.get(path);
        }
        if self.failed.contains_key(path) {
            return None;
        }

        // 结果到了？
        let ready = {
            let map = self.results.lock().ok()?;
            map.get(path).cloned()
        };
        if let Some(res) = ready {
            match res {
                Ok(img) => {
                    let name = format!("img:{}", path.display());
                    let tex = ctx.load_texture(name, (*img).clone(), TextureOptions::LINEAR);
                    self.textures.insert(path.to_path_buf(), tex);
                    return self.textures.get(path);
                }
                Err(e) => {
                    self.failed.insert(path.to_path_buf(), e);
                    return None;
                }
            }
        }

        if self.requested.insert(path.to_path_buf()) {
            let results = self.results.clone();
            let ctx = ctx.clone();
            let path = path.to_path_buf();
            std::thread::spawn(move || {
                let res = load_image(&path);
                if let Ok(mut map) = results.lock() {
                    map.insert(path, res);
                }
                ctx.request_repaint();
            });
        }
        None
    }

    pub fn failure(&self, path: &Path) -> Option<&String> {
        self.failed.get(path)
    }

    pub fn is_pending(&self, path: &Path) -> bool {
        self.requested.contains(path)
            && !self.textures.contains_key(path)
            && !self.failed.contains_key(path)
    }

    // -----------------------------------------------------------------------
    // 远程图片（http/https）：后台用系统 WinHTTP 取回，主线程只上传纹理
    // -----------------------------------------------------------------------

    pub fn get_url(&mut self, ctx: &egui::Context, url: &str) -> Option<&TextureHandle> {
        if self.url_textures.contains_key(url) {
            return self.url_textures.get(url);
        }
        if self.url_failed.contains_key(url) {
            return None;
        }

        let ready = {
            let map = self.url_results.lock().ok()?;
            map.get(url).cloned()
        };
        if let Some(res) = ready {
            match res {
                Ok(img) => {
                    let tex = ctx.load_texture(
                        format!("url:{url}"),
                        (*img).clone(),
                        TextureOptions::LINEAR,
                    );
                    self.url_textures.insert(url.to_owned(), tex);
                    return self.url_textures.get(url);
                }
                Err(e) => {
                    self.url_failed.insert(url.to_owned(), e);
                    return None;
                }
            }
        }

        if self.url_requested.insert(url.to_owned()) {
            let results = self.url_results.clone();
            let ctx = ctx.clone();
            let url = url.to_owned();
            std::thread::spawn(move || {
                let res = load_url(&url);
                if let Ok(mut map) = results.lock() {
                    map.insert(url, res);
                }
                ctx.request_repaint();
            });
        }
        None
    }

    pub fn url_pending(&self, url: &str) -> bool {
        self.url_requested.contains(url)
            && !self.url_textures.contains_key(url)
            && !self.url_failed.contains_key(url)
    }

    pub fn url_failure(&self, url: &str) -> Option<&String> {
        self.url_failed.get(url)
    }
}

fn load_url(url: &str) -> Loaded {
    let resp = mojuan_core::http::https_get(url, 32 * 1024 * 1024, 12_000)
        .map_err(|e| format!("网络请求失败：{e}"))?;
    if resp.status != 200 {
        return Err(format!("服务器返回 {}", resp.status));
    }
    decode_bytes(&resp.body)
}

fn decode_bytes(bytes: &[u8]) -> Loaded {
    let img = image::load_from_memory(bytes).map_err(|e| format!("图片解码失败：{e}"))?;
    let img = if img.width() > 2200 || img.height() > 2200 {
        img.thumbnail(2200, 2200)
    } else {
        img
    };
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let color = ColorImage::from_rgba_unmultiplied([w as usize, h as usize], rgba.as_raw());
    Ok(Arc::new(color))
}

fn load_image(path: &Path) -> Loaded {
    let bytes = std::fs::read(path).map_err(|e| format!("读取失败：{e}"))?;
    decode_bytes(&bytes)
}
