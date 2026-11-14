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
}

fn load_image(path: &Path) -> Loaded {
    let bytes = std::fs::read(path).map_err(|e| format!("读取失败：{e}"))?;
    let img = image::load_from_memory(&bytes).map_err(|e| format!("解码失败：{e}"))?;

    // 超大图先缩小，控制显存占用
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
