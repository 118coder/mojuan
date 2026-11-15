//! Mermaid `flowchart` / `graph` 的纯逻辑解析器与分层布局。
//!
//! 不依赖 egui、不含任何 UI 代码。UI 层用 `boxes()` 返回的矩形画节点、
//! 用 `edges` 与矩形中心画箭头。只支持最常用的 flowchart / graph，
//! 其它图类型（sequenceDiagram / classDiagram / gantt…）返回 `None`。

use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    TB,
    LR,
    BT,
    RL,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeShape {
    Rect,
    Round,
    Diamond,
    Circle,
}

#[derive(Debug, Clone)]
pub struct MNode {
    pub id: String,
    pub label: String,
    pub shape: NodeShape,
}

#[derive(Debug, Clone)]
pub struct MEdge {
    pub from: String,
    pub to: String,
    pub label: String,
}

#[derive(Debug, Clone)]
pub struct MGraph {
    pub dir: Direction,
    pub nodes: Vec<MNode>,
    pub edges: Vec<MEdge>,
    pub width: f32,
    pub height: f32,
}

const PAD: f32 = 24.0;
const LAYER_GAP: f32 = 84.0;
const NODE_GAP: f32 = 28.0;
const NODE_H: f32 = 36.0;
const DIAMOND_H: f32 = 48.0;

/// 解析 mermaid 源码；不是 flowchart/graph 时返回 `None`。
pub fn parse(src: &str) -> Option<MGraph> {
    // 清理注释与空行，拼成按换行分隔的语句流
    let mut cleaned = String::new();
    for line in src.lines() {
        let line = match line.find("%%") {
            Some(i) => &line[..i],
            None => line,
        };
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        if !cleaned.is_empty() {
            cleaned.push('\n');
        }
        cleaned.push_str(t);
    }

    let statements = split_statements(&cleaned);
    let mut it = statements.into_iter();
    let header = it.next()?; // 第一条语句必须是图头
    let h = header.trim();
    let mut words = h.split_whitespace();
    let kw = words.next()?;
    if !(kw.eq_ignore_ascii_case("flowchart") || kw.eq_ignore_ascii_case("graph")) {
        return None;
    }
    let d = words
        .next()
        .unwrap_or("TB")
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .collect::<String>()
        .to_ascii_uppercase();
    let dir = match d.as_str() {
        "LR" => Direction::LR,
        "BT" => Direction::BT,
        "RL" => Direction::RL,
        _ => Direction::TB, // TD / TB / 缺省
    };

    let mut nodes: Vec<MNode> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut edges: Vec<MEdge> = Vec::new();
    for st in it {
        let t = st.trim();
        if t.is_empty() || ignored(t) {
            continue;
        }
        parse_statement(t, &mut nodes, &mut index, &mut edges);
    }

    // 边两端节点不存在时自动补裸节点（按首次出现顺序去重）
    for e in &edges {
        ensure_node(&mut nodes, &mut index, &e.from, None, None);
        ensure_node(&mut nodes, &mut index, &e.to, None, None);
    }

    let mut g = MGraph {
        dir,
        nodes,
        edges,
        width: 0.0,
        height: 0.0,
    };
    let (_, w, h) = compute_layout(&g);
    g.width = w;
    g.height = h;
    Some(g)
}

/// 按换行或 `;` 切分语句，但忽略方括号/圆括号/花括号/pipe 内部的 `;`，
/// 也把 `F>文本]` 的不对称节点里的 `;` 视为标签内容。
fn split_statements(src: &str) -> Vec<String> {
    let cs: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth: usize = 0;
    let mut in_pipe = false;
    for (i, &c) in cs.iter().enumerate() {
        let prev = if i > 0 { Some(cs[i - 1]) } else { None };
        match c {
            '[' | '(' | '{' => {
                depth += 1;
                cur.push(c);
            }
            ']' | ')' | '}' => {
                if depth > 0 {
                    depth -= 1;
                }
                cur.push(c);
            }
            '|' => {
                if depth == 0 {
                    in_pipe = !in_pipe;
                }
                cur.push(c);
            }
            '>' => {
                // `-->` / `==>` 里的 `>` 前一个字符是 `-`/`=`，不会误判
                if depth == 0 && prev.map_or(false, |p| p.is_alphanumeric() || p == '_') {
                    depth += 1;
                }
                cur.push(c);
            }
            ';' | '\n' if depth == 0 && !in_pipe => {
                let t = cur.trim();
                if !t.is_empty() {
                    out.push(t.to_string());
                }
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    let t = cur.trim();
    if !t.is_empty() {
        out.push(t.to_string());
    }
    out
}

/// subgraph / 样式 / 交互等非节点语句，当作透明容器或直接忽略。
fn ignored(st: &str) -> bool {
    let first = st.split_whitespace().next().unwrap_or("");
    let key = first.to_ascii_lowercase();
    matches!(
        key.as_str(),
        "subgraph" | "end" | "classdef" | "class" | "style" | "linkstyle" | "click"
            | "direction" | "flowchart" | "graph"
    )
}

fn ensure_node(
    nodes: &mut Vec<MNode>,
    index: &mut HashMap<String, usize>,
    id: &str,
    label: Option<String>,
    shape: Option<NodeShape>,
) {
    if let Some(&i) = index.get(id) {
        // 之前是裸节点时，后出现的显式定义补全标签与形状（不改变首次出现顺序）
        if let (Some(l), Some(s)) = (label, shape) {
            if nodes[i].label == nodes[i].id {
                nodes[i].label = l;
                nodes[i].shape = s;
            }
        }
    } else {
        let l = label.unwrap_or_else(|| id.to_string());
        let s = shape.unwrap_or(NodeShape::Rect);
        index.insert(id.to_string(), nodes.len());
        nodes.push(MNode {
            id: id.to_string(),
            label: l,
            shape: s,
        });
    }
}

// ---------------------------------------------------------------------------
// 语句解析
// ---------------------------------------------------------------------------

struct Stmt {
    cs: Vec<char>,
    pos: usize,
}

impl Stmt {
    fn new(s: &str) -> Self {
        Stmt {
            cs: s.chars().collect(),
            pos: 0,
        }
    }

    fn eof(&self) -> bool {
        self.pos >= self.cs.len()
    }

    fn peek(&self) -> Option<char> {
        self.cs.get(self.pos).copied()
    }

    fn peek_at(&self, k: usize) -> Option<char> {
        self.cs.get(self.pos + k).copied()
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(' ' | '\t' | '\r' | '\n')) {
            self.pos += 1;
        }
    }

    fn starts(&self, s: &str) -> bool {
        let cc: Vec<char> = s.chars().collect();
        if self.pos + cc.len() > self.cs.len() {
            return false;
        }
        &self.cs[self.pos..self.pos + cc.len()] == cc.as_slice()
    }

    fn advance(&mut self, n: usize) {
        self.pos = (self.pos + n).min(self.cs.len());
    }

    fn find_from(&self, from: usize, toks: &[&str]) -> Option<(usize, usize)> {
        for i in from..self.cs.len() {
            for tok in toks {
                let cc: Vec<char> = tok.chars().collect();
                if i + cc.len() <= self.cs.len()
                    && &self.cs[i..i + cc.len()] == cc.as_slice()
                {
                    return Some((i, cc.len()));
                }
            }
        }
        None
    }

    fn read_until(&mut self, close: &str) -> String {
        let cc: Vec<char> = close.chars().collect();
        let start = self.pos;
        while self.pos + cc.len() <= self.cs.len() {
            if &self.cs[self.pos..self.pos + cc.len()] == cc.as_slice() {
                let s: String = self.cs[start..self.pos].iter().collect();
                self.pos += cc.len();
                return s;
            }
            self.pos += 1;
        }
        let s: String = self.cs[start..].iter().collect();
        self.pos = self.cs.len();
        s
    }

    /// 解析一个节点引用，返回 (id, 可选标签, 可选形状)。
    fn parse_node(&mut self) -> Option<(String, Option<String>, Option<NodeShape>)> {
        self.skip_ws();
        let start = self.pos;
        while self.pos < self.cs.len() {
            let c = self.cs[self.pos];
            if c.is_alphanumeric() || c == '_' {
                self.pos += 1;
            } else if c == '-' {
                // 不要把 `-->` / `---` / `-.->` 的开头吃进 id
                let nx = self.peek_at(1);
                if matches!(nx, Some('-' | '>' | '.')) || nx.is_none() {
                    break;
                }
                self.pos += 1;
            } else {
                break;
            }
        }
        let id: String = self.cs[start..self.pos].iter().collect();
        if id.is_empty() {
            return None;
        }
        let (label, shape) = self.parse_shape();
        Some((id, label, shape))
    }

    fn parse_shape(&mut self) -> (Option<String>, Option<NodeShape>) {
        let c = match self.peek() {
            Some(c) => c,
            None => return (None, None),
        };
        let (open, close, shape): (usize, &str, NodeShape) = match c {
            '[' => {
                if self.peek_at(1) == Some('[') {
                    (2, "]]", NodeShape::Rect) // [[...]] 子程序形
                } else {
                    (1, "]", NodeShape::Rect)
                }
            }
            '(' => {
                if self.peek_at(1) == Some('(') {
                    (2, "))", NodeShape::Circle) // ((...)) 圆形
                } else {
                    (1, ")", NodeShape::Round)
                }
            }
            '{' => (1, "}", NodeShape::Diamond),
            '>' => (1, "]", NodeShape::Round), // F>文本] 不对称形
            _ => return (None, None),
        };
        self.advance(open);
        let raw = self.read_until(close);
        let t = raw.trim();
        let label = if t.is_empty() {
            None
        } else {
            Some(t.to_string())
        };
        (label, Some(shape))
    }

    fn pipe_label(&mut self) -> String {
        if self.starts("|") {
            self.advance(1);
            return self.read_until("|").trim().to_string();
        }
        String::new()
    }

    fn skip_class_suffix(&mut self) {
        if self.starts(":::") {
            self.advance(3);
            while self.pos < self.cs.len() {
                let c = self.cs[self.pos];
                if c.is_alphanumeric() || c == '_' || c == '-' {
                    self.pos += 1;
                } else {
                    break;
                }
            }
        }
    }

    /// 尝试在当前位置解析一条边，返回边标签；不是边则返回 None。
    fn parse_edge(&mut self) -> Option<String> {
        self.skip_ws();
        self.skip_class_suffix();
        self.skip_ws();
        for tok in ["-.->", "==>", "-->", "---", "->"] {
            if self.starts(tok) {
                self.advance(tok.chars().count());
                return Some(self.pipe_label());
            }
        }
        // 带文本的边：-- 文本 --> / -- 文本 --- / == 文本 ==> / -. 文本 .->
        let (skip, toks): (usize, &[&str]) = if self.starts("--") {
            (2, &["-->", "---"])
        } else if self.starts("==") {
            (2, &["==>"])
        } else if self.starts("-.") {
            (2, &[".->"])
        } else {
            return None;
        };
        let from = self.pos + skip;
        let (end, tlen) = self.find_from(from, toks)?;
        let raw: String = self.cs[from..end].iter().collect();
        self.pos = end + tlen;
        let mut label = raw.trim().to_string();
        let pipe = self.pipe_label();
        if !pipe.is_empty() {
            label = pipe;
        }
        Some(label)
    }
}

fn parse_statement(
    s: &str,
    nodes: &mut Vec<MNode>,
    index: &mut HashMap<String, usize>,
    edges: &mut Vec<MEdge>,
) {
    let mut p = Stmt::new(s);
    let mut prev: Option<String> = None;
    loop {
        if p.eof() {
            break;
        }
        let from = match prev.take() {
            Some(f) => f,
            None => match p.parse_node() {
                Some((id, label, shape)) => {
                    ensure_node(nodes, index, &id, label, shape);
                    prev = Some(id);
                    continue;
                }
                None => break,
            },
        };
        let label = match p.parse_edge() {
            Some(l) => l,
            None => break,
        };
        let (id, nlabel, nshape) = match p.parse_node() {
            Some(n) => n,
            None => break,
        };
        ensure_node(nodes, index, &id, nlabel, nshape);
        edges.push(MEdge {
            from,
            to: id.clone(),
            label,
        });
        prev = Some(id);
    }
}

// ---------------------------------------------------------------------------
// 分层布局
// ---------------------------------------------------------------------------

fn estimate_width(label: &str) -> f32 {
    let mut w = 0.0f32;
    for c in label.chars() {
        // 中文 / 全角约 14px，英文约 7.5px
        w += if c.is_ascii() { 7.5 } else { 14.0 };
    }
    w.clamp(72.0, 260.0)
}

fn node_height(shape: NodeShape) -> f32 {
    if shape == NodeShape::Diamond {
        DIAMOND_H
    } else {
        NODE_H
    }
}

/// Kahn 最长路径分层。环图（回边）会让入度永远清不了零，导致下游全部
/// 卡在第 0 层；所以队列空了还有剩余节点时，把剩余里未定层前驱最少的一个
/// 强制定层（回边只画箭头、不参与层级推算），直到全部节点定层，必然收敛。
fn assign_layers(g: &MGraph) -> Vec<Vec<usize>> {
    let n = g.nodes.len();
    if n == 0 {
        return Vec::new();
    }
    let mut idx: HashMap<String, usize> = HashMap::new();
    for (i, node) in g.nodes.iter().enumerate() {
        idx.insert(node.id.clone(), i);
    }
    let mut succ: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut preds: Vec<Vec<usize>> = vec![Vec::new(); n];
    for e in &g.edges {
        if let (Some(&u), Some(&v)) = (idx.get(&e.from), idx.get(&e.to)) {
            if u != v && !succ[u].contains(&v) {
                succ[u].push(v);
                preds[v].push(u);
            }
        }
    }
    let mut layer = vec![0usize; n];
    let mut done = vec![false; n];
    let mut waiting = preds.iter().map(|p| p.len()).collect::<Vec<_>>();
    let mut placed = 0usize;
    while placed < n {
        // 本轮可定层：所有前驱都已定层
        let frontier: Vec<usize> = (0..n).filter(|&i| !done[i] && waiting[i] == 0).collect();
        let batch: Vec<usize> = if frontier.is_empty() {
            // 剩余全是环：挑一个打破
            vec![(0..n)
                .filter(|&i| !done[i])
                .min_by_key(|&i| waiting[i])
                .unwrap()]
        } else {
            frontier
        };
        for &i in &batch {
            layer[i] = preds[i]
                .iter()
                .filter(|&&p| done[p])
                .map(|&p| layer[p] + 1)
                .max()
                .unwrap_or(0);
        }
        for &i in &batch {
            done[i] = true;
            placed += 1;
            for &v in &succ[i] {
                if !done[v] {
                    waiting[v] -= 1;
                }
            }
        }
    }
    let max_layer = layer.iter().copied().max().unwrap_or(0);
    let mut layers: Vec<Vec<usize>> = vec![Vec::new(); max_layer + 1];
    for i in 0..n {
        layers[layer[i]].push(i);
    }
    layers
}

/// 计算每个节点的外接矩形与画布尺寸。
/// 返回 (rects, width, height)，rects[i] = (x, y, w, h)，x/y 为矩形左上角。
fn compute_layout(g: &MGraph) -> (Vec<(f32, f32, f32, f32)>, f32, f32) {
    let n = g.nodes.len();
    if n == 0 {
        return (Vec::new(), 2.0 * PAD + 48.0, 2.0 * PAD + 48.0);
    }
    let widths: Vec<f32> = g
        .nodes
        .iter()
        .map(|nd| estimate_width(&nd.label))
        .collect();
    let heights: Vec<f32> = g.nodes.iter().map(|nd| node_height(nd.shape)).collect();
    let layers = assign_layers(g);
    let lcount = layers.len();
    let vertical = matches!(g.dir, Direction::TB | Direction::BT);

    let mut layer_extent = Vec::with_capacity(lcount);
    for l in &layers {
        let mut e = 0.0f32;
        for (k, &i) in l.iter().enumerate() {
            if k > 0 {
                e += NODE_GAP;
            }
            e += if vertical { widths[i] } else { heights[i] };
        }
        layer_extent.push(e);
    }
    let max_extent = layer_extent.iter().copied().fold(0.0f32, f32::max);

    // 主轴：层间距 84px；交叉轴：同层节点按出现顺序排开，间距 28px，整体居中
    let mut rects = vec![(0.0f32, 0.0f32, 0.0f32, 0.0f32); n];
    for (j, l) in layers.iter().enumerate() {
        let main = match g.dir {
            Direction::TB | Direction::LR => LAYER_GAP * j as f32,
            Direction::BT | Direction::RL => LAYER_GAP * (lcount - 1 - j) as f32,
        };
        let mut cross = (max_extent - layer_extent[j]) / 2.0;
        for &i in l {
            let (cx, cy) = if vertical {
                let cx = cross + widths[i] / 2.0;
                cross += widths[i] + NODE_GAP;
                (cx, main)
            } else {
                let cy = cross + heights[i] / 2.0;
                cross += heights[i] + NODE_GAP;
                (main, cy)
            };
            rects[i] = (
                cx - widths[i] / 2.0,
                cy - heights[i] / 2.0,
                widths[i],
                heights[i],
            );
        }
    }

    // 归一化：整体平移使最小坐标为 PAD，画布 = 外接框 + 两侧 PAD 内边距
    let mut min_x = f32::MAX;
    let mut max_rx = f32::MIN;
    let mut min_y = f32::MAX;
    let mut max_ry = f32::MIN;
    for &(x, y, w, h) in &rects {
        min_x = min_x.min(x);
        max_rx = max_rx.max(x + w);
        min_y = min_y.min(y);
        max_ry = max_ry.max(y + h);
    }
    let dx = PAD - min_x;
    let dy = PAD - min_y;
    for r in &mut rects {
        r.0 += dx;
        r.1 += dy;
    }
    let width = ((max_rx - min_x) + 2.0 * PAD).max(2.0 * PAD + 48.0);
    let height = ((max_ry - min_y) + 2.0 * PAD).max(2.0 * PAD + 48.0);
    (rects, width, height)
}

impl MGraph {
    /// 返回 (节点下标, x, y, 宽, 高)，坐标相对于画布左上角。
    /// x/y 为矩形左上角，中心为 (x + 宽/2, y + 高/2)；
    /// 所有矩形都落在 [0, width] × [0, height] 内。
    pub fn boxes(&self) -> Vec<(usize, f32, f32, f32, f32)> {
        let (rects, _, _) = compute_layout(self);
        rects
            .into_iter()
            .enumerate()
            .map(|(i, (x, y, w, h))| (i, x, y, w, h))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// 测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_in_bounds(g: &MGraph) {
        assert!(g.width >= 48.0 && g.height >= 48.0);
        assert_eq!(g.boxes().len(), g.nodes.len());
        for (_, x, y, w, h) in g.boxes() {
            assert!(x >= 0.0, "x={x}");
            assert!(y >= 0.0, "y={y}");
            assert!(x + w <= g.width + 0.001, "right={} width={}", x + w, g.width);
            assert!(y + h <= g.height + 0.001, "bottom={} height={}", y + h, g.height);
            assert!(w >= 72.0 - 0.001 && w <= 260.0 + 0.001, "w={w}");
            assert!(h == NODE_H || h == DIAMOND_H, "h={h}");
        }
    }

    #[test]
    fn simple_chain() {
        let g = parse("flowchart TD\nA --> B --> C").unwrap();
        assert_eq!(g.dir, Direction::TB);
        assert_eq!(g.nodes.len(), 3);
        assert_eq!(g.edges.len(), 2);
        assert_eq!(g.edges[0].from, "A");
        assert_eq!(g.edges[0].to, "B");
        assert_eq!(g.edges[1].from, "B");
        assert_eq!(g.edges[1].to, "C");
        let bs = g.boxes();
        assert!(bs[2].2 > bs[0].2, "TB 应竖向分层");
        assert_in_bounds(&g);
    }

    #[test]
    fn shapes_and_diamond() {
        let g = parse(
            "graph LR\nA{是否通过} --> B[结束]\nC(圆角) --> D((圆))\nE[[子程序]] --> F>不对称]",
        )
        .unwrap();
        let shape = |id: &str| g.nodes.iter().find(|n| n.id == id).unwrap().shape;
        assert_eq!(shape("A"), NodeShape::Diamond);
        assert_eq!(shape("B"), NodeShape::Rect);
        assert_eq!(shape("C"), NodeShape::Round);
        assert_eq!(shape("D"), NodeShape::Circle);
        assert_eq!(shape("E"), NodeShape::Rect);
        assert_eq!(shape("F"), NodeShape::Round);
        assert_eq!(g.nodes.iter().find(|n| n.id == "E").unwrap().label, "子程序");
        assert_eq!(g.nodes.iter().find(|n| n.id == "F").unwrap().label, "不对称");
        assert_eq!(g.edges.len(), 3);
        assert_in_bounds(&g);
    }

    #[test]
    fn edge_labels() {
        let g = parse(
            "flowchart TB\nA -- 是 --> B\nA -->|否| C\nB -.-> D\nC ==> E\nD --- F",
        )
        .unwrap();
        assert!(g.edges.iter().any(|e| e.label == "是"), "{:?}", g.edges);
        assert!(g.edges.iter().any(|e| e.label == "否"), "{:?}", g.edges);
        assert_eq!(g.nodes.len(), 6);
        assert_eq!(g.edges.len(), 5);
        assert_in_bounds(&g);
    }

    #[test]
    fn lr_direction() {
        let g = parse("graph LR\nA --> B --> C").unwrap();
        assert_eq!(g.dir, Direction::LR);
        let bs = g.boxes();
        assert!(bs[1].1 > bs[0].1, "LR 应横向分层");
        assert!(bs[2].1 > bs[1].1);
        assert_in_bounds(&g);
    }

    #[test]
    fn chinese_labels_and_semicolons() {
        let g = parse("flowchart TD\n开始[中文标签 很长很长] --> 结束(完成); 结束 --> A[甲; 乙]")
            .unwrap();
        assert_eq!(
            g.nodes.iter().find(|n| n.id == "开始").unwrap().label,
            "中文标签 很长很长"
        );
        assert_eq!(
            g.nodes.iter().find(|n| n.id == "A").unwrap().label,
            "甲; 乙"
        );
        assert_eq!(g.edges.len(), 2);
        assert_in_bounds(&g);
    }

    #[test]
    fn cycle_does_not_hang() {
        let g = parse("graph TB\nA --> B --> C --> A").unwrap();
        assert_eq!(g.nodes.len(), 3);
        assert_eq!(g.edges.len(), 3);
        assert_in_bounds(&g);
    }

    #[test]
    fn back_edge_does_not_collapse_layers() {
        // 回边 D→B 不能把 B 的下游全部压回第 0 层（此前 Kahn 卡死入度导致错层）
        let g = parse(
            "flowchart TD\nA[开始] --> B{是否通过}\nB -->|是| C[发布版本]\nB -->|否| D[回到编辑]\nD --> B\nC --> E((完成))",
        )
        .unwrap();
        assert_in_bounds(&g);
        let (rects, _w, _h) = compute_layout(&g);
        let id = |s: &str| g.nodes.iter().position(|n| n.id == s).unwrap();
        let (a, b, c, d, e) = (id("A"), id("B"), id("C"), id("D"), id("E"));
        let y = |i: usize| rects[i].1;
        assert!(y(a) < y(b) - 1.0, "A 应在 B 上方");
        assert!(y(b) < y(c).min(y(d)) - 1.0, "C/D 应在 B 下方");
        assert!(y(c).min(y(d)) < y(e) - 1.0, "E 应在 C/D 下方");
    }

    #[test]
    fn subgraphs_comments_and_styles_are_transparent() {
        let g = parse(
            "%% 注释\n  flowchart LR\n  %% 另一条注释\n  subgraph 分组\n    A[甲] --> B[乙]\n  end\n  B --> C[丙]\n  classDef foo fill:#f00\n",
        )
        .unwrap();
        assert_eq!(g.nodes.len(), 3);
        assert_eq!(g.edges.len(), 2);
        assert_in_bounds(&g);
    }

    #[test]
    fn same_line_multiple_statements() {
        let g = parse("graph TD; A --> B; B --> C").unwrap();
        assert_eq!(g.nodes.len(), 3);
        assert_eq!(g.edges.len(), 2);
        assert_in_bounds(&g);
    }

    #[test]
    fn unknown_graph_types_return_none() {
        assert!(parse("sequenceDiagram\nAlice->>Bob: Hi").is_none());
        assert!(parse("classDiagram\nA <|-- B").is_none());
        assert!(parse("gantt\ndateFormat YYYY-MM-DD").is_none());
        assert!(parse("").is_none());
        assert!(parse("%% 只有注释").is_none());
        assert!(parse("  hello world  ").is_none());
    }

    #[test]
    fn all_directions() {
        for (kw, want) in [
            ("flowchart TD", Direction::TB),
            ("graph LR", Direction::LR),
            ("flowchart TB", Direction::TB),
            ("graph RL", Direction::RL),
            ("graph BT", Direction::BT),
            ("flowchart LR", Direction::LR),
        ] {
            let g = parse(&format!("{kw}\nA --> B")).unwrap();
            assert_eq!(g.dir, want, "{kw}");
            assert_in_bounds(&g);
        }
    }
}
