//! 设计系统：调色板 + egui 样式。
//!
//! 配色思路：只有黑、白两套主题，全部用中性灰层叠，贴 Windows 11 原生观感
//! （Fluent：#F3F3F3 窗口底 / 白色表面 / #E5E5E5 发丝线 / 系统蓝强调）。
//! 代码配色取 VS Code Light / Dark 的经典方案，Windows 用户一眼熟悉。

use eframe::egui::{
    self, Color32, CornerRadius, FontFamily, FontId, Margin, Stroke, TextStyle,
};
use mojuan_core::{Theme, TokenStyle};

pub const R_SM: u8 = 4;
pub const R_MD: u8 = 8;
pub const R_LG: u8 = 10;

pub fn c(hex: u32) -> Color32 {
    Color32::from_rgb(
        ((hex >> 16) & 0xFF) as u8,
        ((hex >> 8) & 0xFF) as u8,
        (hex & 0xFF) as u8,
    )
}

pub fn alpha(color: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), a)
}

#[derive(Clone, Copy)]
pub struct Tokens {
    pub plain: Color32,
    pub comment: Color32,
    pub keyword: Color32,
    pub string: Color32,
    pub number: Color32,
    pub constant: Color32,
    pub function: Color32,
    pub ty: Color32,
    pub tag: Color32,
    pub attribute: Color32,
    pub variable: Color32,
    pub operator: Color32,
    pub punct: Color32,
    pub invalid: Color32,
}

impl Tokens {
    pub fn color(&self, s: TokenStyle) -> Color32 {
        match s {
            TokenStyle::Plain => self.plain,
            TokenStyle::Comment => self.comment,
            TokenStyle::Keyword => self.keyword,
            TokenStyle::Str => self.string,
            TokenStyle::Number => self.number,
            TokenStyle::Constant => self.constant,
            TokenStyle::Function => self.function,
            TokenStyle::Type => self.ty,
            TokenStyle::Tag => self.tag,
            TokenStyle::Attribute => self.attribute,
            TokenStyle::Variable => self.variable,
            TokenStyle::Operator => self.operator,
            TokenStyle::Punct => self.punct,
            TokenStyle::Invalid => self.invalid,
        }
    }
}

#[allow(dead_code)]
#[derive(Clone)]
pub struct Palette {
    pub theme: Theme,
    pub dark: bool,

    /// 窗口底色
    pub bg: Color32,
    /// 侧栏 / 顶栏
    pub panel: Color32,
    /// 悬浮卡片、弹层
    pub elev: Color32,
    /// 编辑区底色
    pub edit_bg: Color32,
    /// 阅读区底色
    pub read_bg: Color32,
    /// 中央内容卡片底色（Win11 Explorer 风格的层叠面）
    pub content: Color32,

    pub border: Color32,
    pub border_soft: Color32,

    pub text: Color32,
    pub text_dim: Color32,
    pub text_faint: Color32,

    pub accent: Color32,
    pub accent_hover: Color32,
    pub accent_soft: Color32,
    pub link: Color32,

    pub code_bg: Color32,
    pub code_border: Color32,
    pub inline_code_bg: Color32,
    pub quote_bar: Color32,
    pub table_head: Color32,
    pub table_line: Color32,
    pub sel: Color32,
    pub danger: Color32,
    pub check: Color32,

    pub tokens: Tokens,
}

#[allow(dead_code)]
impl Palette {
    pub fn new(theme: Theme) -> Self {
        match theme {
            Theme::Dark => Self {
                theme,
                dark: true,
                // Win11 暗色：#202020 窗口，#282828 面板，#333333 悬浮层
                bg: c(0x202020),
                panel: c(0x282828),
                elev: c(0x2C2C2C),
                edit_bg: c(0x202020),
                read_bg: c(0x202020),
                content: c(0x242424),
                border: c(0x3B3B3B),
                border_soft: c(0x323232),
                text: c(0xFFFFFF),
                text_dim: c(0xC5C5C5),
                text_faint: c(0x8B8B8B),
                accent: c(0x4CC2FF),
                accent_hover: c(0x6ACDFF),
                accent_soft: Color32::from_rgba_unmultiplied(76, 194, 255, 26),
                link: c(0x4CC2FF),
                code_bg: c(0x26292E),
                code_border: c(0x34383E),
                inline_code_bg: c(0x303339),
                quote_bar: c(0x6E6E6E),
                table_head: c(0x292C31),
                table_line: c(0x34383E),
                sel: Color32::from_rgba_unmultiplied(76, 194, 255, 52),
                danger: c(0xF14C4C),
                check: c(0x6CCB5F),
                tokens: Tokens {
                    plain: c(0xD4D4D4),
                    comment: c(0x6A9955),
                    keyword: c(0x569CD6),
                    string: c(0xCE9178),
                    number: c(0xB5CEA8),
                    constant: c(0x4FC1FF),
                    function: c(0xDCDCAA),
                    ty: c(0x4EC9B0),
                    tag: c(0x569CD6),
                    attribute: c(0x9CDCFE),
                    variable: c(0x9CDCFE),
                    operator: c(0xC8C8C8),
                    punct: c(0xA0A0A0),
                    invalid: c(0xF14C4C),
                },
            },
            Theme::Light => Self {
                theme,
                dark: false,
                // Win11 亮色：#F3F3F3 窗口，白色表面，#E5E5E5 发丝线，系统蓝强调
                bg: c(0xF3F3F3),
                panel: c(0xF3F3F3),
                elev: c(0xFFFFFF),
                edit_bg: c(0xFFFFFF),
                read_bg: c(0xFFFFFF),
                content: c(0xFFFFFF),
                border: c(0xE1E4E8),
                border_soft: c(0xEAEDEF),
                text: c(0x1B1B1B),
                text_dim: c(0x5E5E5E),
                text_faint: c(0x949494),
                accent: c(0x0067C0),
                accent_hover: c(0x1B77C8),
                accent_soft: Color32::from_rgba_unmultiplied(0, 103, 192, 22),
                link: c(0x0067C0),
                code_bg: c(0xF6F8FA),
                code_border: c(0xE7EAEF),
                inline_code_bg: c(0xEFF1F4),
                quote_bar: c(0xD5DAE0),
                table_head: c(0xF8FAFC),
                table_line: c(0xE6E9ED),
                sel: Color32::from_rgba_unmultiplied(0, 103, 192, 42),
                danger: c(0xC42B1C),
                check: c(0x0F7B0F),
                tokens: Tokens {
                    plain: c(0x1B1B1B),
                    comment: c(0x767676),
                    keyword: c(0x0067C0),
                    string: c(0xA31515),
                    number: c(0x098658),
                    constant: c(0x0070C1),
                    function: c(0x795E26),
                    ty: c(0x267F99),
                    tag: c(0x800000),
                    attribute: c(0xB3491F),
                    variable: c(0x001080),
                    operator: c(0x333333),
                    punct: c(0x5A5A5A),
                    invalid: c(0xCD3131),
                },
            },
        }
    }

    pub fn bold(&self) -> FontFamily {
        FontFamily::Name(crate::fonts::SANS_BOLD.into())
    }

    pub fn mono(&self) -> FontFamily {
        FontFamily::Monospace
    }

    pub fn mono_bold(&self) -> FontFamily {
        FontFamily::Name(crate::fonts::MONO_BOLD.into())
    }

    /// 正文基准字号对应的 FontId
    pub fn body(&self, size: f32) -> FontId {
        FontId::new(size, FontFamily::Proportional)
    }

    pub fn bold_font(&self, size: f32) -> FontId {
        FontId::new(size, self.bold())
    }

    pub fn mono_font(&self, size: f32) -> FontId {
        FontId::new(size, self.mono())
    }

    pub fn frame_panel(&self) -> egui::Frame {
        egui::Frame::new()
            .fill(self.panel)
            .inner_margin(Margin::same(0))
    }

    pub fn frame_card(&self) -> egui::Frame {
        egui::Frame::new()
            .fill(self.elev)
            .stroke(Stroke::new(1.0, self.border))
            .corner_radius(CornerRadius::same(R_MD))
            .inner_margin(Margin::same(10))
    }

    pub fn apply(&self, ctx: &egui::Context) {
        let pal = self.clone();
        ctx.all_styles_mut(move |style| pal.fill_style(style));
    }

    fn fill_style(&self, style: &mut egui::Style) {

        let mut v = if self.dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };

        v.dark_mode = self.dark;
        v.panel_fill = self.panel;
        v.window_fill = self.elev;
        v.extreme_bg_color = self.edit_bg;
        v.faint_bg_color = self.border_soft;
        v.code_bg_color = self.code_bg;
        v.hyperlink_color = self.link;
        v.window_stroke = Stroke::new(1.0, self.border);
        v.window_corner_radius = CornerRadius::same(R_LG);
        v.menu_corner_radius = CornerRadius::same(R_MD);
        v.selection.bg_fill = self.sel;
        v.selection.stroke = Stroke::new(1.0, self.accent);
        v.text_cursor.stroke = Stroke::new(1.6, self.accent);
        v.override_text_color = Some(self.text);

        v.widgets.noninteractive.bg_fill = self.panel;
        v.widgets.noninteractive.weak_bg_fill = self.panel;
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, self.border);
        v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, self.text);
        v.widgets.noninteractive.corner_radius = CornerRadius::same(R_SM);

        v.widgets.inactive.bg_fill = alpha(self.text, if self.dark { 12 } else { 6 });
        v.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
        v.widgets.inactive.bg_stroke = Stroke::new(1.0, Color32::TRANSPARENT);
        v.widgets.inactive.fg_stroke = Stroke::new(1.0, self.text_dim);
        v.widgets.inactive.corner_radius = CornerRadius::same(R_SM);

        v.widgets.hovered.bg_fill = alpha(self.text, if self.dark { 20 } else { 8 });
        v.widgets.hovered.weak_bg_fill = alpha(self.text, if self.dark { 20 } else { 8 });
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::TRANSPARENT);
        v.widgets.hovered.fg_stroke = Stroke::new(1.0, self.text);
        v.widgets.hovered.corner_radius = CornerRadius::same(R_SM);

        v.widgets.active.bg_fill = self.accent_soft;
        v.widgets.active.weak_bg_fill = self.accent_soft;
        v.widgets.active.bg_stroke = Stroke::new(1.0, Color32::TRANSPARENT);
        v.widgets.active.fg_stroke = Stroke::new(1.0, self.text);
        v.widgets.active.corner_radius = CornerRadius::same(R_SM);

        v.widgets.open.bg_fill = self.elev;
        v.widgets.open.weak_bg_fill = self.elev;
        v.widgets.open.bg_stroke = Stroke::new(1.0, self.border);
        v.widgets.open.fg_stroke = Stroke::new(1.0, self.text);
        v.widgets.open.corner_radius = CornerRadius::same(R_SM);

        style.visuals = v;

        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(9.0, 4.0);
        style.spacing.menu_margin = Margin::same(6);
        style.spacing.window_margin = Margin::same(10);
        style.spacing.indent = 18.0;
        style.spacing.interact_size.y = 26.0;
        style.spacing.icon_width = 16.0;
        style.spacing.scroll.bar_width = 9.0;
        style.spacing.scroll.floating = true;
        style.spacing.scroll.floating_width = 5.0;
        style.spacing.scroll.handle_min_length = 44.0;
        style.spacing.scroll.bar_inner_margin = 2.0;
        style.spacing.scroll.bar_outer_margin = 2.0;
        style.spacing.scroll.foreground_color = true;
        style.spacing.scroll.dormant_background_opacity = 0.0;
        style.spacing.scroll.interact_background_opacity = 0.10;
        style.spacing.scroll.active_background_opacity = 0.14;
        style.spacing.scroll.dormant_handle_opacity = 0.0;
        style.spacing.scroll.interact_handle_opacity = 0.35;
        style.spacing.scroll.active_handle_opacity = 0.55;
        style.interaction.selectable_labels = true;

        style.text_styles.insert(
            TextStyle::Body,
            FontId::new(14.0, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Button,
            FontId::new(13.0, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Small,
            FontId::new(12.0, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Heading,
            FontId::new(20.0, self.bold()),
        );
        style.text_styles.insert(
            TextStyle::Monospace,
            FontId::new(13.0, FontFamily::Monospace),
        );

    }
}
