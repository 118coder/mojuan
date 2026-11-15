//! 把常见 LaTeX 数学写法降级成可渲染的带样式文本片段。
//!
//! 纯逻辑模块：不依赖 egui、不含任何 UI 代码。UI 层按 `MathSpan.style`
//! 用 egui 绘制（Italic 斜体、Super/Sub 缩小并偏移、Symbol/Operator 用符号字体等）。
//! `render_unicode` 给出可复制 / 可搜索 / 无样式兜底的单行纯文本。
//!
//! 解析为单次线性扫描，分组括号用迭代深度处理（不递归），命令参数递归有深度上限，
//! 因此对畸形输入（未闭合括号、空 `{}`、连续反斜杠）不会 panic 或死循环。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathStyle {
    Normal,
    Italic,
    Bold,
    Super,
    Sub,
    Symbol,
    Operator,
}

#[derive(Debug, Clone)]
pub struct MathSpan {
    pub text: String,
    pub style: MathStyle,
}

#[derive(Debug, Clone)]
pub struct MathLayout {
    pub spans: Vec<MathSpan>,
    /// true = `$$` 块级公式
    pub display: bool,
    /// 原始 LaTeX，复制时用
    pub source: String,
}

fn span(text: impl Into<String>, style: MathStyle) -> MathSpan {
    MathSpan {
        text: text.into(),
        style,
    }
}

/// 解析 `$...$` 里那段内容。
pub fn parse_inline(src: &str) -> MathLayout {
    let mut p = Parser::new(src);
    MathLayout {
        spans: p.parse_seq(Ctx::Normal, false),
        display: false,
        source: src.to_string(),
    }
}

/// 解析 `$$...$$` 里那段内容。
pub fn parse_block(src: &str) -> MathLayout {
    let mut p = Parser::new(src);
    MathLayout {
        spans: p.parse_seq(Ctx::Normal, false),
        display: true,
        source: src.to_string(),
    }
}

/// 把公式转成尽量可读的 Unicode 单行文本（复制、纯文本搜索、无样式兜底）。
///
/// Super / Sub 片段尽量用 Unicode 上/下标字符；无法映射时退化为 `^(...)` / `_(...)`。
/// 内部换行（`\\`、aligned 分行）压成空格，保证结果为单行。
pub fn render_unicode(layout: &MathLayout) -> String {
    let mut out = String::with_capacity(layout.source.len() + 8);
    for sp in &layout.spans {
        match sp.style {
            MathStyle::Super => push_script(&mut out, &sp.text, true),
            MathStyle::Sub => push_script(&mut out, &sp.text, false),
            _ => {
                for ch in sp.text.chars() {
                    out.push(if ch == '\n' { ' ' } else { ch });
                }
            }
        }
    }
    out
}

fn push_script(out: &mut String, s: &str, sup: bool) {
    let has_unmappable = s
        .chars()
        .any(|ch| !ch.is_whitespace() && script_char(ch, sup).is_none());
    if has_unmappable {
        out.push(if sup { '^' } else { '_' });
        out.push('(');
        for ch in s.chars() {
            out.push(if ch == '\n' { ' ' } else { ch });
        }
        out.push(')');
    } else {
        for ch in s.chars() {
            if ch.is_whitespace() {
                out.push(' ');
            } else if let Some(m) = script_char(ch, sup) {
                out.push(m);
            }
        }
    }
}

/// Unicode 上/下标字符映射；需求给出的集合为最小集，这里补充了若干常见字母。
fn script_char(ch: char, sup: bool) -> Option<char> {
    Some(match (sup, ch) {
        (true, '0') => '⁰',
        (true, '1') => '¹',
        (true, '2') => '²',
        (true, '3') => '³',
        (true, '4') => '⁴',
        (true, '5') => '⁵',
        (true, '6') => '⁶',
        (true, '7') => '⁷',
        (true, '8') => '⁸',
        (true, '9') => '⁹',
        (true, '+') => '⁺',
        (true, '-') => '⁻',
        (true, '=') => '⁼',
        (true, '(') => '⁽',
        (true, ')') => '⁾',
        (true, 'n') => 'ⁿ',
        (true, 'i') => 'ⁱ',
        (false, '0') => '₀',
        (false, '1') => '₁',
        (false, '2') => '₂',
        (false, '3') => '₃',
        (false, '4') => '₄',
        (false, '5') => '₅',
        (false, '6') => '₆',
        (false, '7') => '₇',
        (false, '8') => '₈',
        (false, '9') => '₉',
        (false, '+') => '₊',
        (false, '-') => '₋',
        (false, '=') => '₌',
        (false, '(') => '₍',
        (false, ')') => '₎',
        (false, 'a') => 'ₐ',
        (false, 'e') => 'ₑ',
        (false, 'o') => 'ₒ',
        (false, 'x') => 'ₓ',
        (false, 'h') => 'ₕ',
        (false, 'i') => 'ᵢ',
        (false, 'j') => 'ⱼ',
        (false, 'k') => 'ₖ',
        (false, 'l') => 'ₗ',
        (false, 'm') => 'ₘ',
        (false, 'n') => 'ₙ',
        (false, 'p') => 'ₚ',
        (false, 'r') => 'ᵣ',
        (false, 's') => 'ₛ',
        (false, 't') => 'ₜ',
        (false, 'u') => 'ᵤ',
        (false, 'v') => 'ᵥ',
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// 解析器
// ---------------------------------------------------------------------------

const MAX_DEPTH: usize = 64;

#[derive(Clone, Copy)]
enum Ctx {
    Normal,
    Super,
    Sub,
}

fn base_style(ctx: Ctx, base: MathStyle) -> MathStyle {
    match ctx {
        Ctx::Normal => base,
        Ctx::Super => MathStyle::Super,
        Ctx::Sub => MathStyle::Sub,
    }
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
    depth: usize,
}

impl Parser {
    fn new(src: &str) -> Self {
        Parser {
            chars: src.chars().collect(),
            pos: 0,
            depth: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    /// 主解析循环。`stop_on_rbrace` 为真时表示正在读一个 `{...}` 分组的内部，
    /// 遇到与之配对的最外层 `}` 就返回（不消费，交由调用者消费）。
    /// 分组括号用局部深度迭代处理，避免深层嵌套导致递归爆栈。
    fn parse_seq(&mut self, ctx: Ctx, stop_on_rbrace: bool) -> Vec<MathSpan> {
        let mut out = Vec::new();
        let mut depth: usize = 0;
        while let Some(c) = self.peek() {
            if c == '}' {
                if depth > 0 {
                    depth -= 1;
                    self.pos += 1; // 丢弃分组符
                    continue;
                }
                if stop_on_rbrace {
                    break;
                }
                self.pos += 1; // 多余的右括号，直接丢弃
                continue;
            }
            if c == '{' {
                self.pos += 1; // 丢弃分组符
                if stop_on_rbrace {
                    depth += 1;
                }
                continue;
            }
            if c == '\\' {
                out.extend(self.parse_command(ctx));
                continue;
            }
            if c == '^' || c == '_' {
                self.pos += 1;
                let next = if c == '^' { Ctx::Super } else { Ctx::Sub };
                out.extend(self.parse_arg(next));
                continue;
            }
            if c == '&' {
                self.pos += 1;
                out.push(span(" ", MathStyle::Normal)); // 对齐符当空格
                continue;
            }
            self.pos += 1;
            let (text, style) = classify_char(c, ctx);
            out.push(span(text, style));
        }
        out
    }

    /// 读取一个参数（`{...}` 或一个 token）并解析成 spans。带递归深度保护。
    fn parse_arg(&mut self, ctx: Ctx) -> Vec<MathSpan> {
        if self.depth >= MAX_DEPTH {
            return Vec::new();
        }
        self.depth += 1;
        let out = match self.peek() {
            None => Vec::new(),
            Some('{') => {
                self.pos += 1;
                let inner = self.parse_seq(ctx, true);
                if self.peek() == Some('}') {
                    self.pos += 1;
                }
                inner
            }
            Some('\\') => self.parse_command(ctx),
            Some(c) => {
                self.pos += 1;
                let (text, style) = classify_char(c, ctx);
                vec![span(text, style)]
            }
        };
        self.depth -= 1;
        out
    }

    /// 读取 `{...}` 中的原始文本（供 `\text` / `\mathrm` / `\begin` 等使用），
    /// 支持嵌套括号与转义，丢弃最外层花括号。未闭合时读到末尾，不会死循环。
    fn read_group_raw(&mut self) -> String {
        if self.peek() != Some('{') {
            return String::new();
        }
        self.pos += 1;
        let mut s = String::new();
        let mut depth: usize = 1;
        while let Some(c) = self.peek() {
            self.pos += 1;
            if c == '\\' {
                s.push(c);
                if let Some(n) = self.peek() {
                    self.pos += 1;
                    s.push(n);
                }
                continue;
            }
            if c == '{' {
                depth += 1;
                s.push(c);
                continue;
            }
            if c == '}' {
                depth -= 1;
                if depth == 0 {
                    break;
                }
                s.push(c);
                continue;
            }
            s.push(c);
        }
        s
    }

    /// 读取到指定结束字符为止的原始文本（不含结束符），用于 `\sqrt[n]`。
    fn read_until(&mut self, end: char) -> String {
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c == end {
                break;
            }
            self.pos += 1;
            s.push(c);
        }
        s
    }

    /// 当前位置是 `\`，解析一条命令。每个分支都会推进 pos。
    fn parse_command(&mut self, ctx: Ctx) -> Vec<MathSpan> {
        self.pos += 1;
        let Some(c) = self.peek() else {
            return vec![span("\\", MathStyle::Normal)]; // 末尾孤立反斜杠
        };
        if c == '\\' {
            self.pos += 1;
            return vec![span("\n", MathStyle::Normal)]; // `\\` 换行
        }
        if !c.is_alphabetic() {
            self.pos += 1;
            return vec![single_char_command(c, ctx)];
        }
        let start = self.pos;
        while self.peek().map_or(false, |ch| ch.is_alphabetic()) {
            self.pos += 1;
        }
        let name: String = self.chars[start..self.pos].iter().collect();
        if self.peek() == Some('*') {
            self.pos += 1; // 忽略 `*` 变体
        }
        // LaTeX 会忽略命令名后的空白，跳过以便 \frac {a}{b} 之类的写法正确。
        while matches!(self.peek(), Some(' ' | '\t' | '\n' | '\r')) {
            self.pos += 1;
        }
        self.dispatch(&name, ctx)
    }

    fn dispatch(&mut self, name: &str, ctx: Ctx) -> Vec<MathSpan> {
        if let Some((sym, base)) = symbol(name) {
            return vec![span(sym, base_style(ctx, base))];
        }
        match name {
            "frac" | "dfrac" | "tfrac" | "cfrac" => self.frac(ctx),
            "sqrt" => self.sqrt(ctx),
            // 直立文字
            "text" | "textrm" | "mathrm" | "operatorname" | "mbox" | "textsf"
            | "texttt" | "textnormal" | "mathcal" | "mathbb" | "mathfrak" | "mathscr" => {
                let raw = self.read_group_raw();
                vec![span(raw, base_style(ctx, MathStyle::Normal))]
            }
            "mathbf" | "textbf" | "bm" | "boldsymbol" => {
                let raw = self.read_group_raw();
                vec![span(raw, base_style(ctx, MathStyle::Bold))]
            }
            // 重音 / 向量记号：用 Unicode 组合字符
            "hat" | "widehat" | "bar" | "overline" | "vec" | "tilde" | "widetilde"
            | "dot" | "ddot" | "check" | "breve" | "acute" | "grave" => self.accent(name, ctx),
            // 尺寸 / 定界命令，直接丢弃；`\left.` / `\right.` 的不可见定界符也吃掉
            "left" | "right" | "middle" | "big" | "Big" | "bigg" | "Bigg" | "bigl"
            | "bigr" | "Bigl" | "Bigr" | "biggl" | "biggr" | "Biggl" | "Biggr" => {
                if matches!(name, "left" | "right" | "middle") && self.peek() == Some('.') {
                    self.pos += 1;
                }
                Vec::new()
            }
            // 水平空白
            "quad" | "qquad" | "enspace" | "thinspace" | "medspace" | "thickspace" => {
                vec![span(" ", MathStyle::Normal)]
            }
            "hspace" => {
                self.read_group_raw();
                vec![span(" ", MathStyle::Normal)]
            }
            // 环境名去掉；内容里的 `\\` / `&` 已由主循环处理
            "begin" | "end" => {
                self.read_group_raw();
                Vec::new()
            }
            // 各类不影响降级文本的修饰符
            "displaystyle" | "textstyle" | "scriptstyle" | "scriptscriptstyle"
            | "limits" | "nolimits" | "nonumber" | "notag" | "label" | "tag" | "bf" | "rm"
            | "it" | "sf" | "tt" | "allowbreak" | "noindent" => Vec::new(),
            // 常见函数名 / 运算符名，直立输出
            "sin" | "cos" | "tan" | "cot" | "sec" | "csc" | "arcsin" | "arccos" | "arctan"
            | "sinh" | "cosh" | "tanh" | "log" | "ln" | "lg" | "exp" | "lim" | "limsup"
            | "liminf" | "max" | "min" | "sup" | "inf" | "det" | "dim" | "gcd" | "deg"
            | "arg" | "Pr" | "mod" | "bmod" | "pmod" => {
                vec![span(name, base_style(ctx, MathStyle::Normal))]
            }
            // 未知命令：保留反斜杠原样
            _ => vec![span(format!("\\{}", name), MathStyle::Normal)],
        }
    }

    fn frac(&mut self, ctx: Ctx) -> Vec<MathSpan> {
        let a = self.parse_arg(ctx);
        let b = self.parse_arg(ctx);
        let mut out = Vec::with_capacity(a.len() + b.len() + 5);
        out.push(span("(", base_style(ctx, MathStyle::Normal)));
        out.extend(a);
        out.push(span(")", base_style(ctx, MathStyle::Normal)));
        out.push(span("/", base_style(ctx, MathStyle::Operator)));
        out.push(span("(", base_style(ctx, MathStyle::Normal)));
        out.extend(b);
        out.push(span(")", base_style(ctx, MathStyle::Normal)));
        out
    }

    fn sqrt(&mut self, ctx: Ctx) -> Vec<MathSpan> {
        let mut out = Vec::new();
        if self.peek() == Some('[') {
            self.pos += 1;
            let raw = self.read_until(']');
            if self.peek() == Some(']') {
                self.pos += 1;
            }
            if !raw.is_empty() {
                out.push(span(raw, MathStyle::Super)); // render_unicode → ⁿ√(x)
            }
        }
        out.push(span("√(", base_style(ctx, MathStyle::Operator)));
        out.extend(self.parse_arg(ctx));
        out.push(span(")", base_style(ctx, MathStyle::Normal)));
        out
    }

    fn accent(&mut self, name: &str, ctx: Ctx) -> Vec<MathSpan> {
        let combining = match name {
            "hat" | "widehat" => "\u{0302}",
            "bar" | "overline" => "\u{0304}",
            "vec" => "\u{20D7}",
            "tilde" | "widetilde" => "\u{0303}",
            "dot" => "\u{0307}",
            "ddot" => "\u{0308}",
            "check" => "\u{030C}",
            "breve" => "\u{0306}",
            "acute" => "\u{0301}",
            "grave" => "\u{0300}",
            _ => "",
        };
        let inner = self.parse_arg(ctx);
        let style = inner
            .first()
            .map(|s| s.style)
            .unwrap_or(base_style(ctx, MathStyle::Italic));
        let mut text = String::new();
        for sp in &inner {
            text.push_str(&sp.text);
        }
        text.push_str(combining);
        vec![span(text, style)]
    }
}

fn classify_char(c: char, ctx: Ctx) -> (String, MathStyle) {
    let base = if c.is_ascii_alphabetic() {
        MathStyle::Italic
    } else if c.is_ascii_digit() {
        MathStyle::Normal
    } else if matches!(c, '+' | '-' | '=' | '<' | '>' | '*' | '/' | '|') {
        MathStyle::Operator
    } else {
        MathStyle::Normal
    };
    let text = match c {
        '\n' | '\r' | '\t' => " ".to_string(),
        _ => c.to_string(),
    };
    (text, base_style(ctx, base))
}

fn single_char_command(c: char, ctx: Ctx) -> MathSpan {
    let text = match c {
        '{' | '}' | '%' | '&' | '#' | '$' | '_' => c.to_string(),
        ',' | ';' | ':' | '!' | ' ' => " ".to_string(),
        _ => format!("\\{}", c), // 未知单字符命令保留反斜杠
    };
    span(text, base_style(ctx, MathStyle::Normal))
}

/// 希腊字母与数学符号 → Unicode。返回 (字符, 基准样式)。
fn symbol(name: &str) -> Option<(&'static str, MathStyle)> {
    use MathStyle::*;
    let v = match name {
        // 小写希腊字母
        "alpha" => ("α", Symbol),
        "beta" => ("β", Symbol),
        "gamma" => ("γ", Symbol),
        "delta" => ("δ", Symbol),
        "epsilon" | "varepsilon" => ("ε", Symbol),
        "zeta" => ("ζ", Symbol),
        "eta" => ("η", Symbol),
        "theta" => ("θ", Symbol),
        "vartheta" => ("ϑ", Symbol),
        "iota" => ("ι", Symbol),
        "kappa" => ("κ", Symbol),
        "lambda" => ("λ", Symbol),
        "mu" => ("μ", Symbol),
        "nu" => ("ν", Symbol),
        "xi" => ("ξ", Symbol),
        "omicron" => ("ο", Symbol),
        "pi" => ("π", Symbol),
        "varpi" => ("ϖ", Symbol),
        "rho" => ("ρ", Symbol),
        "varrho" => ("ϱ", Symbol),
        "sigma" => ("σ", Symbol),
        "varsigma" => ("ς", Symbol),
        "tau" => ("τ", Symbol),
        "upsilon" => ("υ", Symbol),
        "phi" => ("ϕ", Symbol),
        "varphi" => ("φ", Symbol),
        "chi" => ("χ", Symbol),
        "psi" => ("ψ", Symbol),
        "omega" => ("ω", Symbol),
        // 大写希腊字母（含大写变体）
        "Alpha" => ("Α", Symbol),
        "Beta" => ("Β", Symbol),
        "Gamma" => ("Γ", Symbol),
        "Delta" => ("Δ", Symbol),
        "Epsilon" => ("Ε", Symbol),
        "Zeta" => ("Ζ", Symbol),
        "Eta" => ("Η", Symbol),
        "Theta" => ("Θ", Symbol),
        "Iota" => ("Ι", Symbol),
        "Kappa" => ("Κ", Symbol),
        "Lambda" => ("Λ", Symbol),
        "Mu" => ("Μ", Symbol),
        "Nu" => ("Ν", Symbol),
        "Xi" => ("Ξ", Symbol),
        "Omicron" => ("Ο", Symbol),
        "Pi" => ("Π", Symbol),
        "Rho" => ("Ρ", Symbol),
        "Sigma" => ("Σ", Symbol),
        "Tau" => ("Τ", Symbol),
        "Upsilon" => ("Υ", Symbol),
        "Phi" => ("Φ", Symbol),
        "Chi" => ("Χ", Symbol),
        "Psi" => ("Ψ", Symbol),
        "Omega" => ("Ω", Symbol),
        // 大运算符
        "sum" => ("∑", Operator),
        "prod" => ("∏", Operator),
        "int" => ("∫", Operator),
        "oint" => ("∮", Operator),
        "iint" => ("∬", Operator),
        "iiint" => ("∭", Operator),
        "bigcup" => ("⋃", Operator),
        "bigcap" => ("⋂", Operator),
        "bigvee" => ("⋁", Operator),
        "bigwedge" => ("⋀", Operator),
        // 二元运算符
        "pm" => ("±", Operator),
        "mp" => ("∓", Operator),
        "times" => ("×", Operator),
        "cdot" => ("·", Operator),
        "div" => ("÷", Operator),
        "ast" => ("∗", Operator),
        "star" => ("⋆", Operator),
        "circ" => ("∘", Operator),
        "bullet" => ("∙", Operator),
        "oplus" => ("⊕", Operator),
        "otimes" => ("⊗", Operator),
        "odot" => ("⊙", Operator),
        "wedge" | "land" => ("∧", Operator),
        "vee" | "lor" => ("∨", Operator),
        "setminus" => ("∖", Operator),
        "cap" => ("∩", Operator),
        "cup" => ("∪", Operator),
        // 关系 / 箭头
        "leq" | "le" => ("≤", Operator),
        "geq" | "ge" => ("≥", Operator),
        "neq" | "ne" => ("≠", Operator),
        "approx" => ("≈", Operator),
        "equiv" => ("≡", Operator),
        "sim" => ("∼", Operator),
        "simeq" => ("≃", Operator),
        "cong" => ("≅", Operator),
        "propto" => ("∝", Operator),
        "ll" => ("≪", Operator),
        "gg" => ("≫", Operator),
        "subset" => ("⊂", Operator),
        "supset" => ("⊃", Operator),
        "subseteq" => ("⊆", Operator),
        "supseteq" => ("⊇", Operator),
        "in" => ("∈", Operator),
        "notin" => ("∉", Operator),
        "ni" => ("∋", Operator),
        "to" | "rightarrow" => ("→", Operator),
        "leftarrow" => ("←", Operator),
        "Rightarrow" => ("⇒", Operator),
        "Leftarrow" => ("⇐", Operator),
        "leftrightarrow" => ("↔", Operator),
        "Leftrightarrow" => ("⇔", Operator),
        "mapsto" => ("↦", Operator),
        "uparrow" => ("↑", Operator),
        "downarrow" => ("↓", Operator),
        "mid" => ("∣", Operator),
        "parallel" => ("∥", Operator),
        "perp" => ("⊥", Operator),
        // 其它符号
        "infty" => ("∞", Symbol),
        "partial" => ("∂", Symbol),
        "nabla" => ("∇", Symbol),
        "emptyset" | "varnothing" => ("∅", Symbol),
        "forall" => ("∀", Symbol),
        "exists" => ("∃", Symbol),
        "neg" | "lnot" => ("¬", Symbol),
        "angle" => ("∠", Symbol),
        "ldots" | "dots" => ("…", Symbol),
        "cdots" => ("⋯", Symbol),
        "vdots" => ("⋮", Symbol),
        "ddots" => ("⋱", Symbol),
        "prime" => ("′", Symbol),
        "hbar" => ("ℏ", Symbol),
        "ell" => ("ℓ", Symbol),
        "Re" => ("ℜ", Symbol),
        "Im" => ("ℑ", Symbol),
        "aleph" => ("ℵ", Symbol),
        "top" => ("⊤", Symbol),
        "bot" => ("⊥", Symbol),
        _ => return None,
    };
    Some(v)
}

// ---------------------------------------------------------------------------
// 测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(l: &MathLayout) -> String {
        let mut s = String::new();
        for sp in &l.spans {
            s.push_str(&sp.text);
        }
        s
    }

    #[test]
    fn greek_and_symbols() {
        let l = parse_inline(
            r"\alpha\beta\Gamma\Omega\pi\theta\infty\partial\nabla\sum\prod\int\oint\pm\mp\times\cdot\div\leq\geq\neq\approx\equiv\to\rightarrow\Rightarrow\in\notin\subset\subseteq\cup\cap\emptyset\forall\exists\angle\perp\ldots\cdots",
        );
        let u = render_unicode(&l);
        for s in [
            "α", "β", "Γ", "Ω", "π", "θ", "∞", "∂", "∇", "∑", "∏", "∫", "∮", "±", "∓",
            "×", "·", "÷", "≤", "≥", "≠", "≈", "≡", "→", "⇒", "∈", "∉", "⊂", "⊆", "∪",
            "∩", "∅", "∀", "∃", "∠", "⊥", "…", "⋯",
        ] {
            assert!(u.contains(s), "缺少 {s}，实际：{u}");
        }
        assert!(l
            .spans
            .iter()
            .any(|s| s.style == MathStyle::Symbol && s.text == "α"));
        assert!(l
            .spans
            .iter()
            .any(|s| s.style == MathStyle::Operator && s.text == "≤"));
    }

    #[test]
    fn super_and_sub_scripts() {
        let l = parse_inline(r"x^2 + x^{n+1} + a_1 + \sqrt[n]{y}");
        let u = render_unicode(&l);
        assert!(u.contains("x²"), "{u}");
        assert!(u.contains("ⁿ⁺¹"), "{u}");
        assert!(u.contains("a₁"), "{u}");
        assert!(u.contains("ⁿ√(y)"), "{u}");
        assert!(l.spans.iter().any(|s| s.style == MathStyle::Super));
        assert!(l.spans.iter().any(|s| s.style == MathStyle::Sub));
    }

    #[test]
    fn script_fallback() {
        // 无法映射成 Unicode 上标的字符，退化成 ^(...)
        let l = parse_inline(r"x^{\alpha}");
        let u = render_unicode(&l);
        assert!(u.contains("^(α)"), "{u}");
    }

    #[test]
    fn frac_and_sqrt() {
        let l = parse_inline(r"\frac{a+b}{c} \dfrac{\alpha}{2} \sqrt{x+1} \sqrt[3]{8}");
        let u = render_unicode(&l);
        assert!(u.contains("(a+b)/(c)"), "{u}");
        assert!(u.contains("(α)/(2)"), "{u}");
        assert!(u.contains("√(x+1)"), "{u}");
        assert!(u.contains("³√(8)"), "{u}");
    }

    #[test]
    fn upright_text() {
        let l = parse_inline(r"\text{hello world 中文} \mathrm{d}x \operatorname{sin}(x)");
        let u = render_unicode(&l);
        assert!(u.contains("hello world 中文"), "{u}");
        assert!(u.contains("dx"), "{u}");
        assert!(u.contains("sin"), "{u}");
        assert!(l
            .spans
            .iter()
            .any(|s| s.text == "hello world 中文" && s.style == MathStyle::Normal));
    }

    #[test]
    fn discard_size_commands_groups_and_spacing() {
        let l = parse_inline(
            r"\left( \frac{1}{2} \right) \big| {a+b} \quad \qquad \; \, \\ \text{x}",
        );
        let u = render_unicode(&l);
        assert!(u.contains("(1)/(2)"), "{u}");
        assert!(u.contains("a+b"), "{u}");
        assert!(!u.contains("left"), "{u}");
        assert!(!u.contains("right"), "{u}");
        assert!(!u.contains("big"), "{u}");
        assert!(!u.contains("quad"), "{u}");
        assert!(!u.contains('{'), "{u}");
        assert!(!u.contains('}'), "{u}");
        assert!(l.spans.iter().any(|s| s.text == "\n"), "应有换行片段");
        assert!(!u.contains('\n'), "render_unicode 应为单行");
    }

    #[test]
    fn aligned_and_matrix_environments() {
        let src = r"\begin{aligned} a &= b + c \\ d &= e \end{aligned}";
        let l = parse_block(src);
        let u = render_unicode(&l);
        assert!(u.contains("a"), "{u}");
        assert!(u.contains("b + c"), "{u}");
        assert!(u.contains("d"), "{u}");
        assert!(!u.contains("aligned"), "{u}");
        assert!(!u.contains("begin"), "{u}");
        assert!(l.spans.iter().any(|s| s.text == "\n"));
        assert!(l.display);
        assert_eq!(l.source, src);

        let m = parse_inline(r"\begin{matrix} 1 & 2 \\ 3 & 4 \end{matrix}");
        assert!(render_unicode(&m).contains("1"));
        assert!(!render_unicode(&m).contains("matrix"));
    }

    #[test]
    fn accents_and_hats() {
        let l = parse_inline(r"\hat{x} \bar{y} \vec{v} \tilde{z}");
        let u = render_unicode(&l);
        assert!(u.contains("x\u{0302}"), "{u:?}");
        assert!(u.contains("y\u{0304}"), "{u:?}");
        assert!(u.contains("v\u{20D7}"), "{u:?}");
        assert!(u.contains("z\u{0303}"), "{u:?}");
    }

    #[test]
    fn unknown_command_is_preserved() {
        let l = parse_inline(r"\foobar x + \alpha");
        let u = render_unicode(&l);
        assert!(u.contains(r"\foobar"), "{u}");
        assert!(u.contains('α'), "{u}");
    }

    #[test]
    fn style_classification() {
        let l = parse_inline("ax + 12 = (b)");
        assert!(l
            .spans
            .iter()
            .any(|s| s.text == "a" && s.style == MathStyle::Italic));
        assert!(l
            .spans
            .iter()
            .any(|s| s.text == "1" && s.style == MathStyle::Normal));
        assert!(l
            .spans
            .iter()
            .any(|s| s.text == "+" && s.style == MathStyle::Operator));
    }

    #[test]
    fn malformed_inputs_do_not_panic_or_hang() {
        let cases = [
            r"\frac{1}{2",     // 未闭合分组
            "x^{",             // 未闭合上标
            "{}",              // 空分组
            "\\",              // 末尾孤立反斜杠
            "^",               // 孤立 ^
            "_",               // 孤立 _
            "a}}}}b",          // 多余右括号
            r"\text{",         // 未闭合 text
            r"\sqrt[3{x",      // 畸形可选参数
            r"\begin{aligned", // 缺 end
            "& & &",
            "{{{{",
            r"\\\\
",
            r"\hat{} \frac{}{}",
        ];
        for c in cases {
            let l = parse_inline(c);
            let _ = render_unicode(&l);
            let _ = plain(&l);
        }
    }

    #[test]
    fn large_input_is_fast_and_robust() {
        let mut src = String::new();
        for _ in 0..2000 {
            src.push_str(r"\alpha + x^2 + \frac{1}{y} ");
        }
        assert!(src.len() > 10_000);
        let l = parse_inline(&src);
        assert!(l.spans.len() > 1000);
        assert!(render_unicode(&l).contains('α'));

        // 深层嵌套花括号：迭代处理，不递归爆栈
        let nested = format!("{}x{}", "{".repeat(5000), "}".repeat(5000));
        let l2 = parse_inline(&nested);
        assert!(render_unicode(&l2).contains('x'));

        // 深层嵌套命令：递归有上限，不 panic
        let deep = format!("{}y{}", r"\frac{".repeat(200), "}{1}".repeat(200));
        let l3 = parse_inline(&deep);
        assert!(!l3.spans.is_empty());
    }
}
