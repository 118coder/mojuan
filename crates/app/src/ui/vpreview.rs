//! 虚拟滚动阅读区：千万字文档的预览只解析、只布局屏幕上看得见的块。
//!
//! 关键点：
//! 1. `VIndex` 是一次线性扫描得到的字节偏移表，不建 AST；
//! 2. 每块高度用 Fenwick 树维护，支持 O(log n) 的「y → 块」定位；
//! 3. `ScrollArea::show_viewport` 只渲染视口附近的块，前后各预载一屏；
//! 4. 每个渲染过的块把实测高度写回，估算值会在一两帧内收敛；
//! 5. 解析结果按块下标做 LRU 缓存，滚动回访不再重复调 comrak。

use std::collections::{HashMap, VecDeque};
use std::path::Path;

use eframe::egui::{Pos2, Rect, ScrollArea, Sense, Ui, Vec2};
use mojuan_core::doc::{Block, Doc};
use mojuan_core::large;
use mojuan_core::vindex::{HeightTree, VIndex, VKind};

use crate::theme::{alpha, Palette};
use crate::ui::{blocks_extra, preview};

/// 超过这个字节数就走虚拟预览（普通文档仍用完整解析，排版最精确）
pub const VIRTUAL_THRESHOLD: usize = 180_000;

const PRELOAD: f32 = 700.0;
const TOP_PAD: f32 = 30.0;
const BOTTOM_PAD: f32 = 160.0;
const CACHE_MAX: usize = 320;

#[derive(Default)]
pub struct State {
    pub index: Option<VIndex>,
    heights: Vec<f32>,
    tree: Option<HeightTree>,
    rev: u64,
    size: f32,
    width: f32,
    cache: HashMap<usize, Vec<Block>>,
    order: VecDeque<usize>,
}

impl State {
    /// 只在「文本变了」时重扫全文；字号/宽度变化只缩放已有高度（不碰文本）。
    pub fn prepare(&mut self, text: &str, size: f32, width: f32, rev: u64) -> &VIndex {
        self.ensure(text, size, width, rev);
        self.index.as_ref().unwrap()
    }

    fn ensure(&mut self, text: &str, size: f32, width: f32, rev: u64) {
        if self.index.is_none() || self.rev != rev {
            let idx = VIndex::build(text);
            self.heights = (0..idx.blocks.len())
                .map(|i| idx.estimate(i, size, width))
                .collect();
            self.tree = Some(HeightTree::new(&self.heights));
            self.index = Some(idx);
            self.cache.clear();
            self.order.clear();
            self.rev = rev;
            self.size = size;
            self.width = width;
            return;
        }
        if (self.size - size).abs() > 0.01 || (self.width - width).abs() > 1.0 {
            // 高度随字号线性变化，随宽度按 sqrt 近似（行折返数量）
            let k = size / self.size.max(1.0)
                * (self.width.max(1.0) / width.max(1.0)).sqrt();
            for h in &mut self.heights {
                *h = (*h * k).max(4.0);
            }
            self.tree = Some(HeightTree::new(&self.heights));
            self.cache.clear();
            self.order.clear();
            self.size = size;
            self.width = width;
        }
    }

    fn parsed(&mut self, i: usize, text: &str, base: Option<&Path>, plain: bool) -> Vec<Block> {
        if let Some(v) = self.cache.get(&i) {
            return v.clone();
        }
        let idx = self.index.as_ref().unwrap();
        let slice = idx.slice(text, i);
        let doc: Doc = if plain {
            mojuan_core::doc::parse_plain(slice)
        } else {
            mojuan_core::doc::parse(slice, base)
        };
        let blocks = doc.blocks;
        self.cache.insert(i, blocks.clone());
        self.order.push_back(i);
        while self.cache.len() > CACHE_MAX {
            if let Some(old) = self.order.pop_front() {
                self.cache.remove(&old);
            } else {
                break;
            }
        }
        blocks
    }

    fn total(&self) -> f32 {
        self.tree.as_ref().map(|t| t.total()).unwrap_or(0.0)
    }

    fn prefix(&self, i: usize) -> f32 {
        self.tree.as_ref().map(|t| t.prefix(i)).unwrap_or(0.0)
    }

    fn find(&self, y: f32) -> usize {
        self.tree.as_ref().map(|t| t.find(y)).unwrap_or(0)
    }

    fn measure(&mut self, i: usize, h: f32) {
        if i >= self.heights.len() {
            return;
        }
        let h = h.max(4.0);
        if (self.heights[i] - h).abs() > 0.6 {
            self.heights[i] = h;
            if let Some(t) = self.tree.as_mut() {
                t.set(i, h);
            }
        }
    }
}

/// 返回 (动作列表, 本次滚动偏移)
#[allow(clippy::too_many_arguments)]
pub fn render(
    state: &mut State,
    ui: &mut Ui,
    pal: &Palette,
    opts: &preview::Opts,
    text: &str,
    base: Option<&Path>,
    rev: u64,
    path_id: &str,
    pending: &mut Option<String>,
    imgs: &mut crate::img::Loader,
    restore: Option<f32>,
    dev_scroll: Option<f32>,
    plain: bool,
) -> (Vec<preview::Action>, f32, f32) {
    let total_w = ui.available_width();
    let col_w = if opts.compact {
        (total_w - 48.0).max(240.0)
    } else {
        opts.measure.min((total_w - 48.0).max(280.0))
    };
    let pad = ((total_w - col_w) * 0.5).max(24.0);

    state.ensure(text, opts.size, col_w, rev);
    let n = state.index.as_ref().map(|i| i.blocks.len()).unwrap_or(0);
    if n == 0 {
        ui.add_space(40.0);
        return (Vec::new(), 0.0, 0.0);
    }

    // 目录跳转：把锚点换算成 y，直接设滚动偏移，不必先渲染那一块
    let mut jump: Option<f32> = None;
    if let Some(target) = pending.clone() {
        if let Some(idx) = state.index.as_ref() {
            if let Some(i) = large::heading_block_by_id(text, idx, &target) {
                jump = Some(state.prefix(i));
                *pending = None;
            }
        }
    }

    let mut area = ScrollArea::vertical()
        .id_salt(("read-v", path_id))
        .auto_shrink([false, false]);
    if let Some(y) = dev_scroll.or(jump).or(restore) {
        // 偏移超出内容总高会落在「first > last」的空档里，整屏空白；
        // 首帧视口高度未知，用估算总高兜底钳到内容末尾。
        let max_y = (state.total() - 400.0).max(0.0);
        area = area.vertical_scroll_offset(y.min(max_y));
    }

    let mut acts = Vec::new();
    let pal2 = pal.clone();
    let base2 = base.map(|p| p.to_path_buf());
    let outer = ui.available_rect_before_wrap();

    let out = area.show_viewport(ui, |ui, viewport| {
        let total = state.total();
        ui.set_height(total + TOP_PAD + BOTTOM_PAD);
        ui.set_width(total_w);

        let y0 = (viewport.min.y - TOP_PAD - PRELOAD).max(0.0);
        let y1 = (viewport.max.y - TOP_PAD + PRELOAD).min(total);
        let first = state.find(y0);
        let last = state.find(y1).min(n - 1);

        // 顶部占位：跳过所有不可见块
        ui.add_space(TOP_PAD + state.prefix(first));

        for i in first..=last {
            let y_before = ui.cursor().min.y;
            let kind = state.index.as_ref().unwrap().blocks[i].kind;

            let mut block_acts = Vec::new();
            ui.horizontal_top(|ui| {
                ui.add_space(pad);
                ui.vertical(|ui| {
                    ui.set_width(col_w);
                    match (plain, kind) {
                        (_, VKind::Math | VKind::Mermaid | VKind::FrontMatter | VKind::Html)
                            if !plain =>
                        {
                            let idx = state.index.as_ref().unwrap();
                            let slice = idx.slice(text, i);
                            block_acts =
                                blocks_extra::render(ui, &pal2, opts, kind, slice, imgs);
                        }
                        _ => {
                            let blocks = state.parsed(i, text, base2.as_deref(), plain);
                            let mut none = None;
                            block_acts = preview::render_blocks(
                                ui,
                                &pal2,
                                opts,
                                &blocks,
                                &mut none,
                                imgs,
                            );
                        }
                    }
                });
                ui.add_space(pad);
            });
            acts.extend(block_acts);

            let y_after = ui.cursor().min.y;
            state.measure(i, y_after - y_before);
        }

        let rest = (total - state.prefix(last + 1)).max(0.0);
        ui.add_space(rest + BOTTOM_PAD);
    });

    let offset = out.state.clamped_offset().y;
    // 右侧极细的阅读进度条：可点击/拖拽跳转
    let total = state.total().max(outer.height());
    let thumb = (outer.height() * outer.height() / total).max(26.0).min(outer.height());
    let span = (outer.height() - thumb).max(1.0);
    let frac = (offset / (total - outer.height()).max(1.0)).clamp(0.0, 1.0);
    let bar = Rect::from_min_size(
        Pos2::new(outer.max.x - 6.0, outer.min.y),
        Vec2::new(6.0, outer.height()),
    );
    let bar_resp = ui.allocate_rect(bar, Sense::click() | Sense::drag());
    if let Some(pos) = bar_resp.interact_pointer_pos() {
        let f = ((pos.y - bar.min.y) / bar.height()).clamp(0.0, 1.0);
        if bar_resp.dragged() || bar_resp.clicked() {
            acts.push(preview::Action::ScrollTo(f * (total - outer.height())));
        }
    }
    ui.painter().rect_filled(
        Rect::from_min_size(
            Pos2::new(outer.max.x - 3.0, outer.min.y + span * frac),
            Vec2::new(2.0, thumb),
        ),
        1,
        alpha(pal.accent, 110),
    );

    (acts, offset, total)
}
