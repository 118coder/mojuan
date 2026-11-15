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

pub enum Dialog {
    NewFile { dir: PathBuf, name: String },
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
}

impl App {
    pub fn new(cc: &eframe::CreationContext) -> Self {
        crate::fonts::install(&cc.egui_ctx);
        let store = Store::load();
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
        };

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
            self.open_path(path);
            self.toast("已打开文件", now);
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
                (true, false, false, Key::F) => {
                    self.side = SideTab::Search;
                    self.store.settings.sidebar_open = true;
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
                }
                _ => {}
            }
        }
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
            egui::CentralPanel::default()
                .frame(egui::Frame::new().fill(self.pal.bg))
                .show(ui, |ui| self.ui_minimal_content(ui, now));
            self.ui_overlays(&ctx, now);
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

        egui::Panel::bottom("statusbar")
            .exact_size(26.0)
            .frame(
                egui::Frame::new()
                    .fill(self.pal.panel)
                    .inner_margin(Margin::symmetric(12, 0)),
            )
            .show(ui, |ui| self.ui_statusbar(ui, now));

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

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(self.pal.bg))
            .show(ui, |ui| {
                self.ui_main(ui, now);
            });

        self.ui_overlays(&ctx, now);
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
        self.persist();
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
