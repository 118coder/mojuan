//! 应用主体：状态、业务逻辑、整体布局。

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use eframe::egui::{
    self, Align, Context, CornerRadius, CursorIcon, Id, Key, Margin, Pos2, Rect, RichText,
    Sense, Stroke, Ui, Vec2,
};
use mojuan_core::{doc, files, search, Theme, ViewMode};
use mojuan_core::files::Entry;
use mojuan_core::search::SearchResult;
use mojuan_core::store::Store;

use crate::theme::{alpha, Palette};
use crate::ui::widgets;

// ---------------------------------------------------------------------------
// 数据结构
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SideTab {
    Files,
    Outline,
    Search,
    Recent,
    Favorite,
}

pub struct Tab {
    pub path: PathBuf,
    pub title: String,
    pub text: String,
    pub crlf: bool,
    pub dirty: bool,
    /// 纯文本（txt）：不按 Markdown 解析与着色
    pub plain: bool,
    pub doc: doc::Doc,
    pub needs_parse: bool,
    pub last_edit: f64,
    pub scroll_edit: f32,
    pub restore_edit: Option<f32>,
    pub scroll_read: f32,
    pub restore_read: Option<f32>,
    pub cursor_line: usize,
    pub cursor_col: usize,
    pub mtime: Option<SystemTime>,
    /// 文本修订号：每次编辑 +1，虚拟预览据此判断是否需要重建索引
    pub rev: u64,
    /// 文件已被外部修改（dirty 时不自动重载，仅提示一次）
    pub ext_stale: bool,
    /// 阅读区内容总高 / 视口高（状态栏百分比用）
    pub read_total: f32,
    pub read_view: f32,
    /// 总行数缓存（0 = 未计算）
    pub line_count: usize,
    /// 大文档虚拟预览状态（每个标签独立）
    pub vpreview: crate::ui::vpreview::State,
    /// 行起始字节表（大文档编辑窗口用；懒构建）
    pub line_starts: Vec<usize>,
    pub line_starts_rev: u64,
    // 大文档编辑滑动窗口
    pub win_text: String,
    pub win_start: usize,
    pub win_end: usize,
    pub win_first_line: usize,
    pub win_valid: bool,
}

impl Tab {
    pub fn new(path: PathBuf, text: String, crlf: bool) -> Self {
        let title = files::display_name(&path);
        let plain = files::ext_of(&path) == "txt";
        Self {
            path,
            title,
            text,
            crlf,
            plain,
            dirty: false,
            doc: doc::Doc::default(),
            needs_parse: true,
            last_edit: 0.0,
            scroll_edit: 0.0,
            restore_edit: None,
            scroll_read: 0.0,
            restore_read: None,
            cursor_line: 1,
            cursor_col: 1,
            mtime: None,
            rev: 1,
            ext_stale: false,
            read_total: 0.0,
            read_view: 0.0,
            line_count: 0,
            vpreview: crate::ui::vpreview::State::default(),
            line_starts: Vec::new(),
            line_starts_rev: 0,
            win_text: String::new(),
            win_start: 0,
            win_end: 0,
            win_first_line: 0,
            win_valid: false,
        }
    }
}

pub struct Workspace {
    pub root: PathBuf,
    pub children: HashMap<PathBuf, Vec<Entry>>,
    pub expanded: HashSet<PathBuf>,
}

impl Workspace {
    pub fn new(root: PathBuf) -> Self {
        let mut expanded = HashSet::new();
        expanded.insert(root.clone());
        let mut children = HashMap::new();
        children.insert(root.clone(), files::list_dir(&root));
        Self {
            root,
            children,
            expanded,
        }
    }

    pub fn ensure(&mut self, dir: &Path) {
        if !self.children.contains_key(dir) {
            self.children
                .insert(dir.to_path_buf(), files::list_dir(dir));
        }
    }

    pub fn refresh(&mut self, dir: &Path) {
        self.children.insert(dir.to_path_buf(), files::list_dir(dir));
    }

    pub fn refresh_all(&mut self) {
        let dirs: Vec<PathBuf> = self.children.keys().cloned().collect();
        for d in dirs {
            self.refresh(&d);
        }
    }
}

pub struct Row {
    pub path: PathBuf,
    pub name: String,
    pub depth: usize,
    pub is_dir: bool,
    pub expanded: bool,
}

/// 文档内查找（记事本式浮条）
#[derive(Clone)]
pub struct FindState {
    pub query: String,
    pub case_sensitive: bool,
    pub idx: usize,
    pub matches: Vec<usize>,
    pub rev: u64,
}

pub enum Dialog {
    NewFile { dir: PathBuf, name: String },
    Replace { find: String, replace: String },
    Goto { input: String },
    NewFolder { dir: PathBuf, name: String },
    Rename { path: PathBuf, name: String },
    Delete { path: PathBuf, is_dir: bool },
    Settings,
    About,
}

pub struct CtxMenu {
    pub pos: Pos2,
    pub target: Option<PathBuf>,
    pub is_dir: bool,
}

#[derive(Clone)]
pub struct Toast {
    pub text: String,
    pub born: f64,
}

pub struct PaletteUi {
    pub open: bool,
    pub query: String,
    pub sel: usize,
    pub focused: bool,
}

pub struct SearchUi {
    pub query: String,
    pub result: SearchResult,
    pub due: Option<f64>,
    pub ran: bool,
}

pub struct App {
    pub store: Store,
    pub pal: Palette,
    pub tabs: Vec<Tab>,
    pub active: usize,
    pub ws: Option<Workspace>,
    pub side: SideTab,
    pub search: SearchUi,
    pub palette: PaletteUi,
    pub dialog: Option<Dialog>,
    pub menu: Option<CtxMenu>,
    pub ws_menu: bool,
    pub toast: Option<Toast>,
    pub pending_scroll: Option<String>,
    pub pending_line: Option<usize>,
    pub last_save_flash: f64,
    pub palette_files: Vec<(PathBuf, String)>,
    pub pending_theme: Option<Theme>,
    pub now: f64,
    pub titlebar_theme: Option<Theme>,
    pub dev_scroll: Option<f32>,
    pub images: crate::img::Loader,
    pub updater: crate::update::Updater,
    pub more_menu: bool,
    /// 极简模式：无侧栏/状态栏/标签条，打开即见内容（类似记事本）
    pub minimal: bool,
    /// 标签右键菜单（下标 + 弹出位置）
    pub tab_menu: Option<(usize, Pos2)>,
    /// 全屏状态（F11）
    pub fullscreen: bool,
    /// 图片放大查看（阅读区点击图片）
    pub viewer: Option<String>,
    /// 最近一帧的窗口位置与内容尺寸（退出时持久化）
    pub win_rect: Option<[f32; 4]>,
    /// Ctrl+F 后自动聚焦搜索输入框（一次性标记）
    pub search_focus: bool,
    /// 文件树滚动定位当前文件（一次性标记）
    pub tree_scroll_req: bool,
    /// 大纲过滤关键词
    pub outline_filter: String,
    /// 图片查看器缩放
    pub viewer_scale: f32,
    /// 文档内查找浮条（Ctrl+F）
    pub find: Option<FindState>,
    /// 内容区右键菜单（是否编辑区 + 弹出位置）
    pub content_menu: Option<(bool, Pos2)>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext) -> Self {
        crate::fonts::install(&cc.egui_ctx);
        let mut store = Store::load();
        // 主题跟随系统明暗（Win11 深色模式）
        if store.settings.theme_auto {
            store.settings.theme = if system_prefers_dark() {
                Theme::Dark
            } else {
                Theme::Light
            };
        }
        let pal = Palette::new(store.settings.theme);
        pal.apply(&cc.egui_ctx);
        cc.egui_ctx.set_theme(match store.settings.theme {
            Theme::Dark => egui::ThemePreference::Dark,
            Theme::Light => egui::ThemePreference::Light,
        });

        let mut app = Self {
            store,
            pal,
            tabs: Vec::new(),
            active: 0,
            ws: None,
            side: SideTab::Files,
            search: SearchUi {
                query: String::new(),
                result: SearchResult::default(),
                due: None,
                ran: false,
            },
            palette: PaletteUi {
                open: false,
                query: String::new(),
                sel: 0,
                focused: false,
            },
            dialog: None,
            menu: None,
            ws_menu: false,
            toast: None,
            pending_scroll: None,
            pending_line: None,
            last_save_flash: 0.0,
            palette_files: Vec::new(),
            pending_theme: None,
            now: 0.0,
            titlebar_theme: None,
            dev_scroll: std::env::var("MJ_SCROLL").ok().and_then(|v| v.parse().ok()),
            images: crate::img::Loader::default(),
            updater: crate::update::Updater::default(),
            more_menu: false,
            minimal: false,
            tab_menu: None,
            fullscreen: false,
            viewer: None,
            win_rect: None,
            search_focus: false,
            tree_scroll_req: false,
            outline_filter: String::new(),
            viewer_scale: 1.0,
            find: None,
            content_menu: None,
        };

        // 恢复上次窗口位置与尺寸（多显示器离屏兜底）
        if let Some([x, y, w, h]) = app.store.settings.window {
            if x > -2000.0 && y > -2000.0 && x < 20000.0 && y < 20000.0 && w > 220.0 && h > 200.0 {
                cc.egui_ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::Pos2::new(x, y)));
                cc.egui_ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::Vec2::new(w, h)));
            }
        }
        // 恢复上次的工作区 / 文件
        if let Some(ws) = app.store.settings.last_workspace.clone() {
            let p = PathBuf::from(&ws);
            if p.is_dir() {
                app.open_workspace(p);
            }
        }
        if let Some(f) = app.store.settings.last_file.clone() {
            let p = PathBuf::from(&f);
            if p.is_file() {
                app.open_path(p);
                app.restore_last_scroll();
            }
        }
        // 开发期快捷入口：MJ_UI=outline|search|recent|palette|about|settings|read|edit
        if let Ok(ui) = std::env::var("MJ_UI") {
            match ui.as_str() {
                "outline" => app.side = SideTab::Outline,
                "search" => {
                    app.side = SideTab::Search;
                    app.store.settings.sidebar_open = true;
                    app.search.query = std::env::var("MJ_QUERY").unwrap_or_else(|_| "设计".into());
                    app.run_search();
                }
                "recent" => app.side = SideTab::Recent,
                "palette" => {
                    app.refresh_palette_files();
                    app.palette.open = true;
                }
                "about" => app.dialog = Some(Dialog::About),
                "settings" => app.dialog = Some(Dialog::Settings),
                "update" => {
                    let repo = app.updater.repo(&app.store.settings.update_repo);
                    app.updater.check(repo, 0.0);
                    app.updater.dialog_open = true;
                }
                "read" => app.store.settings.mode = ViewMode::Read,
                "edit" => app.store.settings.mode = ViewMode::Edit,
                _ => {}
            }
        }
        app
    }

    // -- 基础 -------------------------------------------------------------

    pub fn tab(&self) -> Option<&Tab> {
        self.tabs.get(self.active)
    }

    pub fn tab_mut(&mut self) -> Option<&mut Tab> {
        self.tabs.get_mut(self.active)
    }

    pub fn active_path(&self) -> Option<PathBuf> {
        self.tab().map(|t| t.path.clone())
    }

    pub fn toast(&mut self, msg: impl Into<String>, now: f64) {
        self.toast = Some(Toast {
            text: msg.into(),
            born: now,
        });
    }

    pub fn set_theme(&mut self, ctx: &Context, theme: Theme) {
        self.store.settings.theme = theme;
        // 手动切换 = 用户明确表态，退出「跟随系统」模式
        self.store.settings.theme_auto = false;
        self.pal = Palette::new(theme);
        self.pal.apply(ctx);
        ctx.set_theme(match theme {
            Theme::Dark => egui::ThemePreference::Dark,
            Theme::Light => egui::ThemePreference::Light,
        });
        // 重新解析文档以刷新代码配色（token 颜色在渲染时取用，无需重解析）
        let _ = self.store.save();
    }

    pub fn set_mode(&mut self, mode: ViewMode) {
        self.store.settings.mode = mode;
    }

    pub fn persist(&mut self) {
        self.store.settings.last_file = self.active_path().map(|p| p.to_string_lossy().to_string());
        self.store.settings.window = self.win_rect;
        self.store.settings.last_scroll = self.tab().map(|t| t.scroll_read).unwrap_or(0.0);
        let _ = self.store.save();
    }

    // -- 文件 -------------------------------------------------------------

    pub fn open_workspace(&mut self, root: PathBuf) {
        self.ws = Some(Workspace::new(root.clone()));
        self.store.touch_workspace(&root);
        let _ = self.store.save();
        if self.side == SideTab::Recent {
            self.side = SideTab::Files;
        }
        // 自动打开第一个 md
        if self.tabs.is_empty() {
            if let Some(ws) = &self.ws {
                if let Some(list) = ws.children.get(&root) {
                    if let Some(first) = list.iter().find(|e| !e.is_dir) {
                        let p = first.path.clone();
                        self.open_path(p);
                    }
                }
            }
        }
    }

    pub fn pick_workspace(&mut self, now: f64) {
        if let Some(dir) = rfd::FileDialog::new().set_title("选择工作区文件夹").pick_folder() {
            self.open_workspace(dir);
            self.toast("已打开工作区", now);
        }
    }

    pub fn pick_open_file(&mut self, now: f64) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Markdown", &["md", "markdown", "mdown", "txt"])
            .set_title("打开 Markdown 文件")
            .pick_file()
        {
            self.open_path(path.clone());
            let name = files::display_name(&path);
            self.toast(format!("已打开「{name}」"), now);
        }
    }

    pub fn open_path(&mut self, path: PathBuf) {
        if let Some(i) = self.tabs.iter().position(|t| t.path == path) {
            self.active = i;
            self.search_scroll_reset();
            return;
        }
        match files::read_doc(&path) {
            Ok((text, crlf)) => {
                let plain = files::ext_of(&path) == "txt";
                let mut tab = Tab::new(path.clone(), text, crlf);
                tab.mtime = files::mtime(&path);
                self.tabs.push(tab);
                self.active = self.tabs.len() - 1;
                self.store.touch_file(&path);
                // txt 按记事本体验直入编辑
                if plain {
                    self.store.settings.mode = ViewMode::Edit;
                }
                // 没有工作区时，把文件所在目录作为工作区，左侧文件树才有内容
                if self.ws.is_none() {
                    if let Some(parent) = path.parent() {
                        self.ws = Some(Workspace::new(parent.to_path_buf()));
                        self.side = SideTab::Files;
                    }
                }
                // 自动展开文件所在目录链，并让文件树滚动定位
                if let Some(ws) = &mut self.ws {
                    let mut anc = path.parent();
                    while let Some(a) = anc {
                        ws.expanded.insert(a.to_path_buf());
                        if a == ws.root {
                            break;
                        }
                        anc = a.parent();
                    }
                }
                self.tree_scroll_req = true;
                let _ = self.store.save();
                self.search_scroll_reset();
            }
            Err(e) => {
                self.toast(e, self.now);
            }
        }
    }

    fn search_scroll_reset(&mut self) {
        if let Some(t) = self.tab_mut() {
            t.restore_edit = Some(t.scroll_edit);
        }
    }

    pub fn save_active(&mut self, now: f64) -> bool {
        let Some(tab) = self.tab_mut() else { return false };
        if !tab.dirty {
            return true;
        }
        match files::write_doc(&tab.path, &tab.text, tab.crlf) {
            Ok(()) => {
                tab.dirty = false;
                tab.ext_stale = false;
                tab.mtime = files::mtime(&tab.path);
                self.last_save_flash = now;
                self.toast("已保存", now);
                true
            }
            Err(e) => {
                self.toast(e, now);
                false
            }
        }
    }

    /// 中央区域右键 → 打开内容菜单（阅读/编辑按模式与位置判定）
    fn maybe_open_content_menu(&mut self, ctx: &Context, central: Rect, now: f64) {
        if self.viewer.is_some() {
            return;
        }
        let (sec, pos) = ctx.input(|i| (i.pointer.secondary_clicked(), i.pointer.interact_pos()));
        if !sec {
            return;
        }
        let Some(pos) = pos else { return };
        if !central.contains(pos) {
            return;
        }
        // 顶部标签条区域不触发
        if pos.y < central.min.y + 44.0 {
            return;
        }
        let is_editor = match self.store.settings.mode {
            ViewMode::Edit => true,
            ViewMode::Read => false,
            ViewMode::Split => {
                let ratio = self.store.settings.split_ratio.clamp(0.25, 0.75);
                pos.x < central.min.x + central.width() * ratio
            }
        };
        self.content_menu = Some((is_editor, pos));
    }

    pub fn switch_tab(&mut self, idx: usize) {
        let n = self.tabs.len();
        if n == 0 {
            return;
        }
        let i = idx.min(n - 1);
        if let Some(tab) = self.tabs.get_mut(i) {
            tab.restore_edit = Some(tab.scroll_edit);
            tab.restore_read = Some(tab.scroll_read);
        }
        self.active = i;
    }

    pub fn close_tab(&mut self, idx: usize, now: f64) {
        if idx >= self.tabs.len() {
            return;
        }
        if self.tabs[idx].dirty {
            self.save_active(now);
        }
        self.tabs.remove(idx);
        if self.active >= self.tabs.len() && !self.tabs.is_empty() {
            self.active = self.tabs.len() - 1;
        }
        if self.tabs.is_empty() {
            self.active = 0;
        }
    }

    pub fn create_file(&mut self, dir: &Path, name: &str, now: f64) {
        match files::create_file(dir, name) {
            Ok(path) => {
                self.refresh_parent(dir);
                self.open_path(path);
                self.toast("已新建文件", now);
            }
            Err(e) => self.toast(e, now),
        }
    }

    pub fn create_folder(&mut self, dir: &Path, name: &str, now: f64) {
        match files::create_dir(dir, name) {
            Ok(path) => {
                self.refresh_parent(dir);
                if let Some(ws) = &mut self.ws {
                    ws.expanded.insert(path);
                }
                self.toast("已新建文件夹", now);
            }
            Err(e) => self.toast(e, now),
        }
    }

    pub fn rename(&mut self, path: &Path, new_name: &str, now: f64) {
        let old_index = self.tabs.iter().position(|t| t.path == path);
        match files::rename(path, new_name) {
            Ok(new_path) => {
                if let Some(i) = old_index {
                    self.tabs[i].path = new_path.clone();
                    self.tabs[i].title = files::display_name(&new_path);
                }
                if let Some(parent) = path.parent() {
                    self.refresh_parent(parent);
                }
                self.toast("已重命名", now);
            }
            Err(e) => self.toast(e, now),
        }
    }

    pub fn delete(&mut self, path: &Path, now: f64) {
        match files::delete(path) {
            Ok(()) => {
                if let Some(i) = self.tabs.iter().position(|t| t.path == path) {
                    self.tabs.remove(i);
                    if self.active >= self.tabs.len() && !self.tabs.is_empty() {
                        self.active = self.tabs.len() - 1;
                    }
                }
                if let Some(parent) = path.parent() {
                    self.refresh_parent(parent);
                }
                self.store.forget_file(path);
                let _ = self.store.save();
                self.toast("已移入回收站", now);
            }
            Err(e) => self.toast(e, now),
        }
    }

    fn refresh_parent(&mut self, dir: &Path) {
        if let Some(ws) = &mut self.ws {
            ws.refresh(dir);
        }
    }

    // -- 解析 / 搜索 ------------------------------------------------------

    pub fn parse_pending(&mut self, now: f64) {
        let size = self.store.settings.font_size;
        let width = self.store.settings.measure;
        let Some(tab) = self.tab_mut() else { return };
        if tab.needs_parse && now - tab.last_edit > 0.09 {
            let base = tab.path.parent().map(|p| p.to_path_buf());
            if tab.text.len() > crate::ui::vpreview::VIRTUAL_THRESHOLD {
                // 大文档：只建偏移索引 + 目录/统计，绝不生成整篇 AST
                // （txt 也走这条路，渲染时视口内的切片再按纯文本解析）
                let meta = {
                    let idx = tab.vpreview.prepare(&tab.text, size, width, tab.rev);
                    mojuan_core::large::build_meta(&tab.text, idx)
                };
                tab.doc = meta;
            } else if tab.plain {
                tab.doc = doc::parse_plain(&tab.text);
            } else {
                tab.doc = doc::parse(&tab.text, base.as_deref());
            }
            tab.needs_parse = false;
        }
    }

    pub fn run_search(&mut self) {
        let q = self.search.query.trim().to_string();
        if q.is_empty() {
            self.search.result = SearchResult::default();
            self.search.ran = false;
            return;
        }
        self.search.result = match &self.ws {
            Some(ws) => search::search(&ws.root, &q),
            None => SearchResult::default(),
        };
        self.search.ran = true;
    }

    pub fn refresh_palette_files(&mut self) {
        let Some(ws) = &self.ws else {
            self.palette_files.clear();
            return;
        };
        let root = ws.root.clone();
        let files = files::walk_md(&root, 3000);
        self.palette_files = files
            .into_iter()
            .map(|p| {
                let rel = files::rel_display(&root, &p);
                (p, rel)
            })
            .collect();
    }

    pub fn visible_rows(&mut self) -> Vec<Row> {
        let Some(ws) = &mut self.ws else { return Vec::new() };
        let root = ws.root.clone();
        ws.ensure(&root);
        let mut out = Vec::new();
        collect_rows(ws, &root, 0, &mut out);
        out
    }

    pub fn workspace_name(&self) -> String {
        self.ws
            .as_ref()
            .map(|w| files::display_name(&w.root))
            .unwrap_or_else(|| "未打开工作区".to_string())
    }

    // -- 快捷键 -----------------------------------------------------------

    pub fn handle_shortcuts(&mut self, ctx: &Context, now: f64) {
        let mut cmd = false;
        let mut shift = false;
        let mut alt = false;
        let mut keys: Vec<egui::Key> = Vec::new();
        ctx.input(|i| {
            cmd = i.modifiers.command;
            shift = i.modifiers.shift;
            alt = i.modifiers.alt;
            keys = i.events.iter().filter_map(|e| match e {
                egui::Event::Key {
                    key,
                    pressed: true,
                    repeat: false,
                    ..
                } => Some(*key),
                _ => None,
            }).collect();
        });

        if self.dialog.is_some() {
            return;
        }

        for key in keys {
            match (cmd, shift, alt, key) {
                (true, false, false, Key::S) => {
                    self.save_active(now);
                }
                (true, false, false, Key::N) => {
                    let dir = self
                        .tab()
                        .and_then(|t| t.path.parent().map(|p| p.to_path_buf()))
                        .or_else(|| self.ws.as_ref().map(|w| w.root.clone()));
                    if let Some(dir) = dir {
                        self.dialog = Some(Dialog::NewFile {
                            dir,
                            name: "未命名.md".into(),
                        });
                    }
                }
                (true, false, false, Key::O) => self.pick_open_file(now),
                (true, true, false, Key::O) => self.pick_workspace(now),
                (true, false, false, Key::P) => {
                    self.refresh_palette_files();
                    self.palette.open = true;
                    self.palette.query.clear();
                    self.palette.sel = 0;
                    self.palette.focused = false;
                }
                (true, false, false, Key::E) => {
                    let next = match self.store.settings.mode {
                        ViewMode::Read => ViewMode::Split,
                        ViewMode::Split => ViewMode::Edit,
                        ViewMode::Edit => ViewMode::Read,
                    };
                    self.set_mode(next);
                }
                (true, false, false, Key::W) => self.close_tab(self.active, now),
                (true, false, false, Key::Tab) => {
                    let n = self.tabs.len();
                    if n > 0 {
                        self.switch_tab((self.active + 1) % n);
                    }
                }
                (true, true, false, Key::Tab) => {
                    let n = self.tabs.len();
                    if n > 0 {
                        self.switch_tab((self.active + n - 1) % n);
                    }
                }
                (false, false, false, Key::F11) => {
                    self.fullscreen = !self.fullscreen;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
                }
                (true, false, false, Key::Backslash) => {
                    let open = self.store.settings.sidebar_open;
                    self.store.settings.sidebar_open = !open;
                }
                (true, false, false, Key::D) => self.editor_duplicate_line(ctx, now),
                (true, true, false, Key::K) => self.editor_delete_line(ctx, now),
                (false, false, false, Key::Enter) => self.editor_auto_continue(ctx, now),
                (false, false, false, Key::Tab) => self.editor_indent(ctx, now, false),
                (false, true, false, Key::Tab) => self.editor_indent(ctx, now, true),
                (true, false, false, Key::F) => self.find_open(ctx),
                (true, true, false, Key::F) => {
                    self.side = SideTab::Search;
                    self.store.settings.sidebar_open = true;
                    if let Some((a, b)) = self.editor_cursor_chars(ctx) {
                        if b > a {
                            let sel = self
                                .tab()
                                .map(|t| {
                                    let ba = char_to_byte(&t.text, a);
                                    let bb = char_to_byte(&t.text, b);
                                    t.text[ba..bb].to_string()
                                })
                                .unwrap_or_default();
                            if !sel.is_empty() && sel.chars().count() < 120 {
                                self.search.query = sel;
                            }
                        }
                    }
                    self.search_focus = true;
                    self.run_search();
                }
                (false, false, false, Key::F5) => {
                    let ts = crate::win::local_time_str();
                    self.editor_insert_text(ctx, now, &ts);
                }
                (false, false, false, Key::F3) => self.find_move(ctx, 1),
                (true, false, false, Key::F3) | (false, true, false, Key::F3) => {
                    self.find_move(ctx, -1);
                }
                (true, false, false, Key::P) => self.print_doc(now),
                (true, false, false, Key::G) => {
                    self.dialog = Some(Dialog::Goto { input: String::new() });
                }
                (true, false, false, Key::T) => {
                    let ts = crate::win::local_time_str();
                    self.editor_insert_text(ctx, now, &ts);
                }
                (true, true, false, Key::S) => self.save_as(now),
                (true, false, false, Key::H) => {
                    self.dialog = Some(Dialog::Replace {
                        find: String::new(),
                        replace: String::new(),
                    });
                }
                (true, false, false, Key::Comma) => self.dialog = Some(Dialog::Settings),
                (true, false, false, Key::U) => {
                    let repo = self.updater.repo(&self.store.settings.update_repo);
                    self.updater.check(repo, now);
                    self.updater.dialog_open = true;
                }
                (true, false, false, Key::B) => self.wrap_selection(ctx, "**", "**"),
                (true, false, false, Key::I) => self.wrap_selection(ctx, "*", "*"),
                (true, false, false, Key::Equals) => {
                    self.store.settings.font_size = (self.store.settings.font_size + 1.0).min(26.0);
                }
                (true, false, false, Key::Minus) => {
                    self.store.settings.font_size = (self.store.settings.font_size - 1.0).max(12.0);
                }
                (true, false, false, Key::Num0) => {
                    self.store.settings.font_size = 16.0;
                }
                (true, false, false, Key::Home) => {
                    if let Some(t) = self.tab_mut() {
                        t.restore_read = Some(0.0);
                    }
                }
                (true, false, false, Key::End) => {
                    if let Some(t) = self.tab_mut() {
                        t.restore_read = Some(f32::MAX);
                    }
                }
                (false, false, false, Key::Escape) => {
                    self.menu = None;
                    self.palette.open = false;
                    self.more_menu = false;
                    self.ws_menu = false;
                    self.updater.banner = false;
                    self.updater.dialog_open = false;
                    // 极简模式下 Esc 直接回到完整界面
                    if self.minimal {
                        self.minimal = false;
                    }
                    self.tab_menu = None;
                    self.tab_menu = None;
                    self.viewer = None;
                    self.find = None;
                    self.content_menu = None;
                }
                _ => {}
            }
        }
    }

    fn focused_editor(&self, ctx: &Context) -> bool {
        ctx.memory(|m| m.focused()) == Some(self.editor_id())
    }

    /// 编辑器光标/选区（字符下标，a<=b）
    fn editor_cursor_chars(&self, ctx: &Context) -> Option<(usize, usize)> {
        let state = egui::text_edit::TextEditState::load(ctx, self.editor_id())?;
        let range = state.cursor.char_range()?;
        let a = range.primary.index.min(range.secondary.index).0;
        let b = range.primary.index.max(range.secondary.index).0;
        Some((a, b))
    }

    fn editor_store_cursor(&self, ctx: &Context, a: usize, b: usize) {
        if let Some(mut st) = egui::text_edit::TextEditState::load(ctx, self.editor_id()) {
            st.cursor.set_char_range(Some(egui::text::CCursorRange::two(
                egui::text::CCursor::new(a),
                egui::text::CCursor::new(b),
            )));
            st.store(ctx, self.editor_id());
        }
    }

    /// 编辑器选区文本（无选区返回空）
    fn editor_selected_text(&self, ctx: &Context) -> Option<String> {
        let (a, b) = self.editor_cursor_chars(ctx)?;
        if a == b {
            return None;
        }
        self.tab().map(|t| {
            let ba = char_to_byte(&t.text, a);
            let bb = char_to_byte(&t.text, b);
            t.text[ba..bb].to_string()
        })
    }

    /// 右键「剪切」：复制选区并删除
    pub fn editor_cut(&mut self, ctx: &Context, now: f64) {
        let Some((a, b)) = self.editor_cursor_chars(ctx) else { return };
        if a == b {
            return;
        }
        let text = self.editor_selected_text(ctx).unwrap_or_default();
        ctx.copy_text(text);
        let idx = self.active;
        let Some(tab) = self.tabs.get_mut(idx) else { return };
        let ba = char_to_byte(&tab.text, a);
        let bb = char_to_byte(&tab.text, b);
        tab.text.replace_range(ba..bb, "");
        self.editor_touch(idx, now);
        self.editor_store_cursor(ctx, a, a);
    }

    /// 右键「复制」：复制选区
    pub fn editor_copy_sel(&mut self, ctx: &Context, now: f64) {
        if let Some(text) = self.editor_selected_text(ctx) {
            ctx.copy_text(text);
            self.toast("已复制选中文本", now);
        }
    }

    /// 右键「粘贴」：替换选区或在光标插入
    pub fn editor_paste(&mut self, ctx: &Context, now: f64) {
        let Ok(text) = arboard::Clipboard::new().and_then(|mut c| c.get_text()) else {
            self.toast("剪贴板里没有文本", now);
            return;
        };
        if text.is_empty() {
            return;
        }
        let (a, b) = self.editor_cursor_chars(ctx).unwrap_or((0, 0));
        let idx = self.active;
        let Some(tab) = self.tabs.get_mut(idx) else { return };
        let ba = char_to_byte(&tab.text, a);
        let bb = char_to_byte(&tab.text, b);
        tab.text.replace_range(ba..bb, &text);
        let nc = tab.text[..ba + text.len()].chars().count();
        self.editor_touch(idx, now);
        self.editor_store_cursor(ctx, nc, nc);
    }

    /// 右键「全选」
    pub fn editor_select_all(&mut self, ctx: &Context) {
        let len = self.tab().map(|t| t.text.chars().count()).unwrap_or(0);
        if len > 0 {
            self.editor_store_cursor(ctx, 0, len);
            ctx.memory_mut(|m| m.request_focus(self.editor_id()));
        }
    }

    /// (行起始, 行末不含换行, 下一行起始含换行)
    fn line_bounds(text: &str, byte: usize) -> (usize, usize, usize) {
        let b = byte.min(text.len());
        let start = text[..b].rfind('\n').map(|i| i + 1).unwrap_or(0);
        let end = text[b..].find('\n').map(|i| b + i).unwrap_or(text.len());
        (start, end, ((end + 1) as u64).min(text.len() as u64) as usize)
    }

    /// 编辑文本统一收口：rev/dirty/needs_parse/撤销窗口失效
    fn editor_touch(&mut self, tab_idx: usize, now: f64) {
        if let Some(t) = self.tabs.get_mut(tab_idx) {
            t.rev = t.rev.wrapping_add(1);
            t.win_valid = false;
            t.dirty = true;
            t.needs_parse = true;
            t.last_edit = now;
            t.line_count = 0;
        }
    }

    /// 光标处插入文本（时间戳等）
    pub fn editor_insert_text(&mut self, ctx: &Context, now: f64, text: &str) {
        if !self.focused_editor(ctx) {
            return;
        }
        let Some((ca, _)) = self.editor_cursor_chars(ctx) else { return };
        let idx = self.active;
        let Some(tab) = self.tabs.get_mut(idx) else { return };
        let byte = char_to_byte(&tab.text, ca);
        tab.text.insert_str(byte, text);
        let nc = tab.text[..byte + text.len()].chars().count();
        self.editor_touch(idx, now);
        self.editor_store_cursor(ctx, nc, nc);
    }

    /// 回车自动续列表：光标行是列表项时接管回车
    pub fn editor_auto_continue(&mut self, ctx: &Context, now: f64) {
        if !self.focused_editor(ctx) {
            return;
        }
        let Some((_, cb)) = self.editor_cursor_chars(ctx) else { return };
        let idx = self.active;
        let line = {
            let Some(tab) = self.tabs.get(idx) else { return };
            let byte = char_to_byte(&tab.text, cb);
            let (ls, le, _) = Self::line_bounds(&tab.text, byte);
            tab.text[ls..le].to_string()
        };
        let trimmed = line.trim_start();
        let indent_len = line.len() - trimmed.len();
        let indent = line[..indent_len].to_string();
        let t = trimmed;
        let marker: Option<String> = if t == "-" || t == "*" || t == "+" {
            None // 空项：退出列表
        } else if t.starts_with("- ")
            || t.starts_with("* ")
            || t.starts_with("+ ")
        {
            Some(format!("{indent}- "))
        } else if let Some((num, rest)) = t.split_once(". ") {
            match num.parse::<u64>() {
                Ok(n) if !rest.is_empty() => Some(format!("{indent}{}. ", n + 1)),
                Ok(_) => None, // 空有序项：退出列表
                Err(_) => return,
            }
        } else {
            return; // 普通行：不接管，交给默认换行
        };
        let consumed = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
        if !consumed {
            return;
        }
        let Some(tab) = self.tabs.get_mut(idx) else { return };
        let byte = char_to_byte(&tab.text, cb);
        let (ls, le, _) = Self::line_bounds(&tab.text, byte);
        match marker {
            Some(prefix) => {
                let ins = format!("\n{prefix}");
                tab.text.insert_str(byte, &ins);
                let nc = tab.text[..byte + ins.len()].chars().count();
                self.editor_touch(idx, now);
                self.editor_store_cursor(ctx, nc, nc);
            }
            None => {
                // 清掉前缀，光标停在本行行首
                tab.text.replace_range(ls..le, "");
                let nc = tab.text[..ls].chars().count();
                self.editor_touch(idx, now);
                self.editor_store_cursor(ctx, nc, nc);
            }
        }
    }

    /// Ctrl+Shift+S：另存为
    pub fn save_as(&mut self, now: f64) {
        let Some(tab) = self.tab() else { return };
        let text = tab.text.clone();
        let crlf = tab.crlf;
        let default_name = tab
            .path
            .file_stem()
            .map(|s| format!("{}.md", s.to_string_lossy()))
            .unwrap_or_else(|| "未命名.md".into());
        if let Some(p) = rfd::FileDialog::new()
            .set_file_name(default_name)
            .add_filter("Markdown", &["md", "markdown", "txt"])
            .save_file()
        {
            match files::write_doc(&p, &text, crlf) {
                Ok(()) => {
                    let name = files::display_name(&p);
                    self.toast(format!("已另存为「{name}」"), now);
                    self.open_path(p);
                }
                Err(e) => self.toast(format!("另存为失败：{e}"), now),
            }
        }
    }

    /// 打开后恢复上次阅读位置
    pub fn restore_last_scroll(&mut self) {
        let y = self.store.settings.last_scroll;
        if y > 1.0 {
            if let Some(t) = self.tabs.last_mut() {
                t.restore_read = Some(y);
            }
        }
    }

    /// 收藏 / 取消收藏当前文档
    /// 复制全文（Markdown 源码）
    pub fn copy_all_md(&mut self, now: f64) {
        if let Some(t) = self.tab() {
            ctx_copy(t.text.clone());
            self.toast("已复制全文（Markdown）", now);
        }
    }

    pub fn toggle_favorite(&mut self, now: f64) {
        let Some(p) = self.active_path() else {
            self.toast("没有打开的文件", now);
            return;
        };
        let key = p.to_string_lossy().to_string();
        let name = files::display_name(&p);
        if let Some(i) = self.store.favorites.iter().position(|s| s == &key) {
            self.store.favorites.remove(i);
            self.toast(format!("已取消收藏「{name}」"), now);
        } else {
            self.store.favorites.push(key);
            self.toast(format!("已收藏「{name}」"), now);
        }
        let _ = self.store.save();
    }

    /// Ctrl+D：复制当前行到下一行
    pub fn editor_duplicate_line(&mut self, ctx: &Context, now: f64) {
        if !self.focused_editor(ctx) {
            return;
        }
        let Some((_, cb)) = self.editor_cursor_chars(ctx) else { return };
        let idx = self.active;
        let Some(tab) = self.tabs.get_mut(idx) else { return };
        let byte = char_to_byte(&tab.text, cb);
        let (ls, le, next) = Self::line_bounds(&tab.text, byte);
        let line = tab.text[ls..le].to_string();
        let insert_at = next.max(le);
        let mut out = String::with_capacity(tab.text.len() + line.len() + 1);
        out.push_str(&tab.text[..insert_at]);
        out.push_str(&line);
        out.push('\n');
        out.push_str(&tab.text[insert_at..]);
        tab.text = out;
        let new_cursor = tab.text[..insert_at + line.len() + 1].chars().count();
        self.editor_touch(idx, now);
        self.editor_store_cursor(ctx, new_cursor, new_cursor);
    }

    /// Ctrl+Shift+K：删除当前行
    pub fn editor_delete_line(&mut self, ctx: &Context, now: f64) {
        if !self.focused_editor(ctx) {
            return;
        }
        let Some((_, cb)) = self.editor_cursor_chars(ctx) else { return };
        let idx = self.active;
        let Some(tab) = self.tabs.get_mut(idx) else { return };
        let byte = char_to_byte(&tab.text, cb);
        let (ls, le, next) = Self::line_bounds(&tab.text, byte);
        let end = if le < next { next } else { le };
        tab.text.replace_range(ls..end, "");
        let cursor = tab.text[..ls].chars().count();
        self.editor_touch(idx, now);
        self.editor_store_cursor(ctx, cursor, cursor);
    }

    /// Tab / Shift+Tab：整块缩进与反缩进
    pub fn editor_indent(&mut self, ctx: &Context, now: f64, outdent: bool) {
        if !self.focused_editor(ctx) {
            return;
        }
        let consumed = ctx.input_mut(|i| {
            if outdent {
                i.consume_key(egui::Modifiers::SHIFT, egui::Key::Tab)
            } else {
                i.consume_key(egui::Modifiers::NONE, egui::Key::Tab)
            }
        });
        if !consumed {
            return;
        }
        let Some((ca, cb)) = self.editor_cursor_chars(ctx) else { return };
        let idx = self.active;
        let Some(tab) = self.tabs.get_mut(idx) else { return };
        let byte_a = char_to_byte(&tab.text, ca);
        let byte_b = char_to_byte(&tab.text, cb);
        let ls1 = Self::line_bounds(&tab.text, byte_a).0;
        let mut ls2 = Self::line_bounds(&tab.text, byte_b).0;
        if ls2 > ls1 && byte_b == ls2 && byte_b > 0 {
            ls2 = Self::line_bounds(&tab.text, byte_b - 1).0;
        }
        let mut starts = Vec::new();
        if ls2 > ls1 {
            let mut scan = ls1;
            while scan <= ls2 {
                starts.push(scan);
                let Some(nl) = tab.text[scan..].find('\n') else { break };
                scan += nl + 1;
            }
        } else {
            starts.push(ls1);
        }
        let mut delta_a = 0i32;
        let mut delta_b = 0i32;
        for &st in starts.iter().rev() {
            let d = if outdent {
                let seg = &tab.text[st..];
                let strip = (seg.len() - seg.trim_start_matches(' ').len()).min(4);
                if strip > 0 {
                    tab.text.replace_range(st..st + strip, "");
                    -(strip as i32)
                } else {
                    0
                }
            } else {
                tab.text.insert_str(st, "    ");
                4
            };
            if st <= byte_a {
                delta_a += d;
            }
            if st <= byte_b {
                delta_b += d;
            }
        }
        let na = (ca as i32 + delta_a).max(0) as usize;
        let nb = (cb as i32 + delta_b).max(0) as usize;
        self.editor_touch(idx, now);
        self.editor_store_cursor(ctx, na, nb);
    }

    // -- 文档内查找（记事本式） ---------------------------------------------

    fn find_matches(text: &str, q: &str, case_sensitive: bool) -> Vec<usize> {
        if q.is_empty() {
            return Vec::new();
        }
        let (hay, needle) = if case_sensitive {
            (text.to_string(), q.to_string())
        } else {
            (text.to_lowercase(), q.to_lowercase())
        };
        if needle.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        let mut start = 0usize;
        while let Some(b) = hay[start..].find(&needle) {
            let abs = start + b;
            out.push(hay[..abs].chars().count());
            start = abs + needle.len();
        }
        out
    }

    /// Ctrl+F：打开/聚焦查找条
    pub fn find_open(&mut self, ctx: &Context) {
        if self.store.settings.mode == mojuan_core::ViewMode::Read {
            self.store.settings.mode = mojuan_core::ViewMode::Split;
        }
        let sel = self.editor_cursor_chars(ctx).and_then(|(a, b)| {
            (b > a).then(|| {
                self.tab().map(|t| {
                    let ba = char_to_byte(&t.text, a);
                    let bb = char_to_byte(&t.text, b);
                    t.text[ba..bb].to_string()
                })
            }).flatten()
        }).unwrap_or_default();
        let (matches, rev) = self.tab().map(|t| {
            (
                Self::find_matches(&t.text, &sel, false),
                t.rev,
            )
        }).unwrap_or((Vec::new(), 0));
        self.find = Some(FindState {
            query: sel,
            case_sensitive: false,
            idx: 0,
            matches,
            rev,
        });
    }

    /// F3 / Shift+F3：下一个 / 上一个
    pub fn find_move(&mut self, ctx: &Context, dir: i32) {
        let Some(f) = &mut self.find else {
            self.find_open(ctx);
            return;
        };
        if f.matches.is_empty() {
            return;
        }
        let n = f.matches.len();
        f.idx = if dir > 0 {
            (f.idx + 1) % n
        } else {
            (f.idx + n - 1) % n
        };
        let i = f.idx;
        let qlen = f.query.chars().count();
        let a = f.matches[i];
        let b = a + qlen;
        self.editor_store_cursor(ctx, a, b);
        if let Some(mut st) = egui::text_edit::TextEditState::load(ctx, self.editor_id()) {
            st.cursor.set_char_range(Some(egui::text::CCursorRange::two(
                egui::text::CCursor::new(a),
                egui::text::CCursor::new(b),
            )));
            st.store(ctx, self.editor_id());
        }
        ctx.memory_mut(|m| m.request_focus(self.editor_id()));
    }

    /// 查找条输入变化后重算
    pub fn find_refresh(&mut self) {
        let (query, case_sensitive, rev) = match &self.find {
            Some(f) => (f.query.clone(), f.case_sensitive, f.rev),
            None => return,
        };
        let Some(t) = self.tab() else { return };
        let matches = Self::find_matches(&t.text, &query, case_sensitive);
        let cur_rev = t.rev;
        drop(t);
        if let Some(f) = &mut self.find {
            f.rev = cur_rev;
            f.matches = matches;
            f.idx = f.idx.min(f.matches.len().saturating_sub(1));
        }
    }

    /// Ctrl+P：打印
    pub fn print_doc(&mut self, now: f64) {
        let title = self.tab().map(|t| t.title.clone()).unwrap_or_default();
        match self.with_text(|text, base| {
            crate::export::print_with_dialog(text, base, &title)
        }) {
            Ok(msg) => self.toast(msg, now),
            Err(e) => self.toast(format!("打印失败：{e}"), now),
        }
    }

    /// 以 ANSI(GBK) 编码另存
    pub fn save_as_gbk(&mut self, now: f64) {
        let Some(tab) = self.tab() else { return };
        let text = tab.text.clone();
        let default_name = tab
            .path
            .file_stem()
            .map(|s| format!("{}.txt", s.to_string_lossy()))
            .unwrap_or_else(|| "未命名.txt".into());
        if let Some(p) = rfd::FileDialog::new()
            .set_file_name(default_name)
            .add_filter("文本文件 (GBK 编码)", &["txt"])
            .save_file()
        {
            match crate::win::utf8_to_gbk(&text) {
                Some(bytes) => match std::fs::write(&p, bytes) {
                    Ok(()) => {
                        let name = files::display_name(&p);
                        self.toast(format!("已按 GBK 编码保存「{name}」"), now);
                    }
                    Err(e) => self.toast(format!("保存失败：{e}"), now),
                },
                None => self.toast("内容包含 GBK 无法表示的字符，已取消", now),
            }
        }
    }

    /// 自动换行开关
    pub fn toggle_wrap(&mut self, now: f64) {
        let w = self.store.settings.editor_wrap;
        self.store.settings.editor_wrap = !w;
        let _ = self.store.save();
        self.toast(
            if w { "已关闭自动换行（横向滚动）" } else { "已开启自动换行" },
            now,
        );
    }

    /// 状态栏开关
    pub fn toggle_status(&mut self, now: f64) {
        let v = self.store.settings.show_status;
        self.store.settings.show_status = !v;
        let _ = self.store.save();
    }

    /// 给选中文本加包裹标记（Ctrl+B / Ctrl+I）
    fn wrap_selection(&mut self, ctx: &Context, left: &str, right: &str) {
        let id = self.editor_id();
        let Some(state) = egui::text_edit::TextEditState::load(ctx, id) else {
            return;
        };
        let Some(range) = state.cursor.char_range() else { return };
        let (a, b) = (
            range.primary.index.min(range.secondary.index),
            range.primary.index.max(range.secondary.index),
        );
        let (a, b) = (a.0, b.0);
        let Some(tab) = self.tab_mut() else { return };
        let text = &tab.text;
        let byte_a = char_to_byte(text, a);
        let byte_b = char_to_byte(text, b);
        let mut new_text = String::with_capacity(text.len() + left.len() + right.len());
        new_text.push_str(&text[..byte_a]);
        new_text.push_str(left);
        new_text.push_str(&text[byte_a..byte_b]);
        new_text.push_str(right);
        new_text.push_str(&text[byte_b..]);
        tab.text = new_text;
        tab.rev = tab.rev.wrapping_add(1);
        tab.win_valid = false;
        tab.dirty = true;
        tab.needs_parse = true;
        tab.last_edit = ctx.input(|i| i.time);

        let mut state = egui::text_edit::TextEditState::load(ctx, id).unwrap_or_default();
        let sel_start = a + left.chars().count();
        let sel_end = sel_start + (b - a);
        state.cursor.set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::new(sel_start),
            egui::text::CCursor::new(sel_end),
        )));
        state.store(ctx, id);
    }

    pub fn editor_id(&self) -> Id {
        match self.tab() {
            Some(t) => Id::unique(("editor", &t.path)),
            None => Id::unique("editor-empty"),
        }
    }

    // -- 滚动跳转 ---------------------------------------------------------

    pub fn jump_to_heading(&mut self, id: String) {
        self.pending_scroll = Some(id);
        if self.store.settings.mode == ViewMode::Edit {
            self.store.settings.mode = ViewMode::Read;
        }
    }

    pub fn jump_to_line(&mut self, path: PathBuf, line: usize) {
        self.open_path(path);
        self.pending_line = Some(line);
        if self.store.settings.mode == ViewMode::Read {
            self.store.settings.mode = ViewMode::Split;
        }
    }

    /// 把内置示例释放到临时目录并作为工作区打开（单文件分发时的"自带说明书"）
    pub fn open_sample(&mut self, now: f64) {
        let dir = std::env::temp_dir().join("mojuan-sample");
        let writes: [(&str, &[u8]); 4] = [
            ("阅读与写作.md", include_bytes!("../assets/sample/阅读与写作.md")),
            ("组件演示.md", include_bytes!("../assets/sample/组件演示.md")),
            ("notes/设计笔记.md", include_bytes!("../assets/sample/notes/设计笔记.md")),
            ("assets/cover.png", include_bytes!("../assets/sample/assets/cover.png")),
        ];
        for (rel, bytes) in writes {
            let path = dir.join(rel);
            if path.exists() {
                continue;
            }
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Err(e) = std::fs::write(&path, bytes) {
                self.toast(format!("释放示例失败：{e}"), now);
                return;
            }
        }
        self.open_workspace(dir);
        self.toast("已打开内置示例", now);
    }

    // -- 交互增强 ----------------------------------------------------------

    /// 拖拽落盘：md/txt 文件新标签打开，文件夹作为工作区
    pub fn handle_drops(&mut self, ctx: &Context, now: f64) {
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        if dropped.is_empty() {
            return;
        }
        let mut files = 0usize;
        let mut dirs = 0usize;
        let mut skipped: Vec<String> = Vec::new();
        for f in &dropped {
            let p = f.path().to_path_buf();
            if p.is_dir() {
                self.open_workspace(p.clone());
                dirs += 1;
            } else if p.is_file() {
                let ext = files::ext_of(&p);
                if files::MD_EXTS.contains(&ext.as_str()) {
                    self.open_path(p.clone());
                    files += 1;
                } else {
                    skipped.push(ext);
                }
            }
        }
        let mut msg = String::new();
        if files > 0 {
            msg.push_str(&format!("已打开 {} 个文件", files));
        }
        if dirs > 0 {
            if !msg.is_empty() {
                msg.push_str("、");
            }
            msg.push_str(&format!("{} 个工作区", dirs));
        }
        if !skipped.is_empty() {
            if !msg.is_empty() {
                msg.push_str("；");
            }
            msg.push_str(&format!(
                "{} 个不支持的文件（{}）",
                skipped.len(),
                skipped.join(", ")
            ));
        }
        if !msg.is_empty() {
            self.toast(msg, now);
        }
    }

    /// 拖拽悬停：全屏提示层
    pub fn draw_drag_overlay(&self, ctx: &Context, _now: f64) {
        let hovering = ctx.input(|i| !i.raw.hovered_files.is_empty());
        if !hovering {
            return;
        }
        let pal = self.pal.clone();
        let screen = ctx
            .input(|i| i.viewport().inner_rect.unwrap_or(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )));
        egui::Area::new(egui::Id::new("drop-overlay"))
            .fixed_pos(screen.min)
            .order(egui::Order::Tooltip)
            .show(ctx, |ui| {
                let r = ui.max_rect();
                ui.painter().rect_filled(r, 0, alpha(pal.accent, 30));
                let inner = r.shrink(22.0);
                ui.painter().rect_stroke(
                    inner,
                    CornerRadius::same(16),
                    Stroke::new(2.0, pal.accent),
                    egui::StrokeKind::Inside,
                );
                let text = "松开即打开 · 文件进新标签，文件夹进工作区";
                let g = ui.painter().layout_no_wrap(
                    text.to_string(),
                    egui::FontId::new(20.0, pal.bold()),
                    pal.text,
                );
                ui.painter().galley(r.center() - g.size() / 2.0, g, pal.text);
            });
    }

    /// Ctrl+滚轮缩放字号
    pub fn handle_wheel_zoom(&mut self, ctx: &Context) {
        let (ctrl, dy) = ctx.input(|i| (i.modifiers.ctrl, i.smooth_scroll_delta.y));
        if !ctrl || dy.abs() < 0.01 {
            return;
        }
        let step = if dy > 0.0 { 1.0 } else { -1.0 };
        self.store.settings.font_size =
            (self.store.settings.font_size + step).clamp(12.0, 26.0);
    }

    /// 外部修改检测：未编辑自动重载；编辑中提示一次
    pub fn check_external_change(&mut self, now: f64) {
        let Some(tab) = self.tab_mut() else { return };
        if tab.dirty || tab.ext_stale {
            return;
        }
        let Some(mt) = files::mtime(&tab.path) else { return };
        let Some(last) = tab.mtime else { return };
        if mt == last {
            return;
        }
        if let Ok((text, crlf)) = files::read_doc(&tab.path) {
            tab.text = text;
            tab.crlf = crlf;
            tab.rev = tab.rev.wrapping_add(1);
            tab.win_valid = false;
            tab.needs_parse = true;
            tab.mtime = files::mtime(&tab.path);
            tab.ext_stale = false;
            let name = tab.title.clone();
            self.toast(format!("「{name}」已被外部修改，已自动重新加载"), now);
        } else {
            tab.ext_stale = true;
        }
    }

    pub fn export_html(&mut self, now: f64) {
        let Some(tab) = self.tab() else { return };
        let title = tab.title.clone();
        let html_body = doc::to_html(&tab.text);
        let html = standalone_html(&title, &html_body, self.store.settings.theme);
        let default_name = tab
            .path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "document".into());
        if let Some(path) = rfd::FileDialog::new()
            .set_file_name(format!("{default_name}.html"))
            .add_filter("HTML", &["html"])
            .save_file()
        {
            match std::fs::write(&path, html) {
                Ok(()) => self.toast("已导出 HTML", now),
                Err(e) => self.toast(format!("导出失败：{e}"), now),
            }
        }
    }

    /// 导出共通：取文本所有权（避免千万字克隆），导完放回
    fn with_text<R>(&mut self, f: impl FnOnce(&str, Option<&Path>) -> Result<R, String>) -> Result<R, String> {
        if self.tabs.is_empty() {
            return Err("没有打开的文档".into());
        }
        let base = self.tabs[self.active].path.parent().map(|p| p.to_path_buf());
        let mut text = std::mem::take(&mut self.tabs[self.active].text);
        let r = f(&text, base.as_deref());
        self.tabs[self.active].text = std::mem::take(&mut text);
        r
    }

    pub fn export_docx(&mut self, now: f64) {
        let default_name = self.tab().map(|t| t.path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "document".into()));
        let Some(default_name) = default_name else { return };
        if let Some(path) = rfd::FileDialog::new()
            .set_file_name(format!("{default_name}.docx"))
            .add_filter("Word 文档", &["docx"])
            .save_file()
        {
            let result = self.with_text(|text, base| crate::export::export_docx(text, base, &default_name, &path));
            match result {
                Ok(msg) => self.toast(msg, now),
                Err(e) => self.toast(format!("导出失败：{e}"), now),
            }
        }
    }

    pub fn export_pdf(&mut self, now: f64) {
        let default_name = self.tab().map(|t| t.path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "document".into()));
        let Some(default_name) = default_name else { return };
        if let Some(path) = rfd::FileDialog::new()
            .set_file_name(format!("{default_name}.pdf"))
            .add_filter("PDF", &["pdf"])
            .save_file()
        {
            let result = self.with_text(|text, base| crate::export::export_pdf(text, base, &default_name, &path));
            match result {
                Ok(msg) => self.toast(msg, now),
                Err(e) => self.toast(format!("导出失败：{e}"), now),
            }
        }
    }

    pub fn export_png(&mut self, now: f64) {
        let default_name = self.tab().map(|t| t.path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "document".into()));
        let Some(default_name) = default_name else { return };
        if let Some(path) = rfd::FileDialog::new()
            .set_file_name(format!("{default_name}.png"))
            .add_filter("PNG 长图", &["png"])
            .save_file()
        {
            let result = self.with_text(|text, base| crate::export::export_png(text, base, &path));
            match result {
                Ok(msg) => self.toast(msg, now),
                Err(e) => self.toast(format!("导出失败：{e}"), now),
            }
        }
    }

    // -- 布局 -------------------------------------------------------------

    pub fn layout(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        self.now = now;

        self.handle_shortcuts(&ctx, now);
        self.handle_drops(ui.ctx(), now);
        self.handle_wheel_zoom(ui.ctx());
        self.check_external_change(now);
        self.find_refresh();
        // 失焦自动保存（autosave 开启时）
        if !ui.ctx().input(|i| i.viewport().focused.unwrap_or(true))
            && self.store.settings.autosave
            && self.tab().map(|t| t.dirty).unwrap_or(false)
        {
            self.save_active(now);
        }
        self.parse_pending(now);

        // 更新检查：轮询后台结果
        let repo = self.updater.repo(&self.store.settings.update_repo);
        let auto = self.store.settings.check_updates;
        let notes = self.updater.poll(now, mojuan_core::VERSION);
        for n in notes {
            self.toast(n, now);
        }
        if !self.updater.auto_done {
            // egui 是"有事件才画"的，定时唤醒一次让自动检查真的会发生
            ctx.request_repaint_after(std::time::Duration::from_secs(2));
            self.updater.maybe_auto(auto, &repo, now);
        }
        if self.updater.busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(120));
        }

        // 搜索防抖
        if let Some(due) = self.search.due {
            if now >= due {
                self.search.due = None;
                self.run_search();
            }
        }

        // 自动保存
        if self.store.settings.autosave {
            if let Some(tab) = self.tab() {
                if tab.dirty && now - tab.last_edit > 1.6 {
                    self.save_active(now);
                }
            }
        }

        // 极简模式：只有一条细工具条 + 内容，双击打开文件时默认走这条路
        if self.minimal && !self.tabs.is_empty() {
            egui::Panel::top("minimal-bar")
                .exact_size(34.0)
                .frame(
                    egui::Frame::new()
                        .fill(self.pal.panel)
                        .inner_margin(Margin::symmetric(10, 0)),
                )
                .show(ui, |ui| self.ui_minimal_bar(ui, now));
            let central = egui::CentralPanel::default()
                .frame(egui::Frame::new().fill(self.pal.content))
                .show(ui, |ui| self.ui_minimal_content(ui, now));
            self.maybe_open_content_menu(&ctx, central.response.rect, now);
            self.ui_overlays(&ctx, now);
            self.draw_drag_overlay(&ctx, now);
            if self.content_menu.is_some() {
                self.ui_content_menu(&ctx, now);
            }
            return;
        }

        egui::Panel::top("topbar")
            .exact_size(44.0)
            .frame(
                egui::Frame::new()
                    .fill(self.pal.panel)
                    .inner_margin(Margin::symmetric(10, 0)),
            )
            .show(ui, |ui| self.ui_topbar(ui, now));

        if self.store.settings.show_status {
        egui::Panel::bottom("statusbar")
            .exact_size(26.0)
            .frame(
                egui::Frame::new()
                    .fill(self.pal.panel)
                    .inner_margin(Margin::symmetric(12, 0)),
            )
            .show(ui, |ui| self.ui_statusbar(ui, now));
        }

        if self.store.settings.sidebar_open {
            egui::Panel::left("sidebar")
                .resizable(true)
                .default_size(self.store.settings.sidebar_width)
                .size_range(210.0..=420.0)
                .frame(
                    egui::Frame::new()
                        .fill(self.pal.panel)
                        .inner_margin(Margin::symmetric(8, 8)),
                )
                .show(ui, |ui| {
                    self.ui_sidebar(ui, now);
                });
        }

        // Win11 Explorer 式内容卡片：侧栏打开时左上圆角
        let card = egui::Frame::new()
            .fill(self.pal.content)
            .corner_radius(egui::CornerRadius {
                nw: if self.store.settings.sidebar_open { 10 } else { 0 },
                ne: 0,
                sw: 0,
                se: 0,
            });
        let central = egui::CentralPanel::default()
            .frame(card)
            .show(ui, |ui| {
                self.ui_main(ui, now);
            });
        self.maybe_open_content_menu(&ctx, central.response.rect, now);

        self.ui_overlays(&ctx, now);
        self.draw_drag_overlay(&ctx, now);
    }
}

fn collect_rows(ws: &mut Workspace, dir: &Path, depth: usize, out: &mut Vec<Row>) {
    if depth > 8 {
        return;
    }
    ws.ensure(dir);
    let Some(entries) = ws.children.get(dir).cloned() else { return };
    for e in entries {
        let expanded = e.is_dir && ws.expanded.contains(&e.path);
        out.push(Row {
            path: e.path.clone(),
            name: e.name.clone(),
            depth,
            is_dir: e.is_dir,
            expanded,
        });
        if expanded {
            collect_rows(ws, &e.path.clone(), depth + 1, out);
        }
    }
}

fn char_to_byte(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}

pub fn standalone_html(title: &str, body: &str, theme: Theme) -> String {
    let scheme = if matches!(theme, Theme::Dark) { "dark" } else { "light" };
    let (bg, fg, dim, border, accent, code_bg, link) = match theme {
        Theme::Dark => ("#202020", "#E6E6E6", "#9D9D9D", "#3D3D3D", "#4CC2FF", "#2B2B2B", "#4CC2FF"),
        Theme::Light => ("#FFFFFF", "#1B1B1B", "#5E5E5E", "#E5E5E5", "#0067C0", "#F6F6F6", "#0067C0"),
    };
    let escaped = html_escape(title);
    format!(
        r#"<!DOCTYPE html>
<html lang="zh-CN"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>{escaped}</title>
<style>
  :root {{ color-scheme: {scheme}; }}
  body {{ background:{bg}; color:{fg}; margin:0; padding:56px 24px 96px;
    font: 16px/1.8 "Microsoft YaHei","PingFang SC","Segoe UI",system-ui,sans-serif; }}
  main {{ max-width: 760px; margin: 0 auto; }}
  h1,h2,h3,h4,h5,h6 {{ line-height:1.35; margin:1.6em 0 .6em; font-weight:700; }}
  h1 {{ font-size:2em; margin-top:0; }}
  h2 {{ font-size:1.5em; padding-bottom:.3em; border-bottom:1px solid {border}; }}
  h3 {{ font-size:1.22em; }}
  p {{ margin:0 0 1.05em; }}
  a {{ color:{link}; text-decoration:none; border-bottom:1px solid {link}55; }}
  code {{ background:{code_bg}; padding:.15em .4em; border-radius:4px;
    font-family:"Cascadia Mono",Consolas,monospace; font-size:.9em; }}
  pre {{ background:{code_bg}; border:1px solid {border}; border-radius:10px; padding:14px 16px; overflow:auto; }}
  pre code {{ background:none; padding:0; font-size:.88em; line-height:1.6; }}
  blockquote {{ margin:1.2em 0; padding:.2em 1em; border-left:3px solid {accent}88; color:{dim}; }}
  table {{ border-collapse:collapse; width:100%; margin:1.2em 0; font-size:.95em; }}
  th,td {{ border:1px solid {border}; padding:8px 12px; text-align:left; }}
  th {{ background:{code_bg}; font-weight:700; }}
  img {{ max-width:100%; border-radius:8px; }}
  hr {{ border:none; border-top:1px solid {border}; margin:2em 0; }}
  ul,ol {{ padding-left:1.4em; }}
  li {{ margin:.3em 0; }}
  footer {{ max-width:760px; margin:48px auto 0; color:{dim}; font-size:12px; }}
</style></head>
<body><main>
{body}
</main><footer>由 墨卷 导出</footer></body></html>"#
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

// ---------------------------------------------------------------------------
// eframe 入口
// ---------------------------------------------------------------------------

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, frame: &mut eframe::Frame) {
        // 记录窗口位置/尺寸，退出时还原
        let (vouter, vinner) = ui.ctx().input(|i| {
            let v = i.viewport();
            (v.outer_rect, v.inner_rect)
        });
        if let (Some(outer), Some(r)) = (vouter, vinner) {
            if outer.min.x.is_finite() && outer.min.y.is_finite() {
                self.win_rect = Some([outer.min.x, outer.min.y, r.width(), r.height()]);
            }
        }
        if self.titlebar_theme != Some(self.store.settings.theme) {
            crate::win::apply(frame, self.pal.dark);
            self.titlebar_theme = Some(self.store.settings.theme);
        }
        crate::devshot::tick(ui.ctx());
        // 让窗口底色与面板一致
        ui.painter().rect_filled(ui.max_rect(), 0, self.pal.bg);
        self.layout(ui);
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        self.persist();
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // 防丢内容：退出时保存所有未保存标签
        for t in &self.tabs {
            if t.dirty {
                let _ = files::write_doc(&t.path, &t.text, t.crlf);
            }
        }
        self.persist();
    }
}

/// 无 Context 时写剪贴板（arboard）
fn ctx_copy(text: String) {
    if let Ok(mut c) = arboard::Clipboard::new() {
        let _ = c.set_text(text);
    }
}

/// 读系统明暗偏好（Win11 设置里的「选择默认 Windows 模式」）
fn system_prefers_dark() -> bool {
    if let Ok(out) = std::process::Command::new("reg")
        .args([
            "query",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
            "/v",
            "AppsUseLightTheme",
        ])
        .output()
    {
        let s = String::from_utf8_lossy(&out.stdout);
        s.contains("0x0")
    } else {
        false
    }
}

// 让未使用的导入不至于报警（保留扩展空间）
#[allow(dead_code)]
fn _unused(pal: &Palette) {
    let _ = alpha(pal.text, 10);
    let _ = CornerRadius::same(4);
    let _ = CursorIcon::PointingHand;
    let _ = Stroke::NONE;
    let _ = Align::LEFT;
    let _ = Sense::hover();
    let _ = widgets::hairline as fn(&mut Ui, &Palette);
    let _ = RichText::new("");
    let _ = Vec2::ZERO;
    let _ = Rect::NOTHING;
}
