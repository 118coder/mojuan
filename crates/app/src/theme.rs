//! 设计系统：调色板 + egui 样式。
//!
//! 配色思路：暗色是「墨与纸」的暖黑，亮色是米白纸，主色用朱砂红，
//! 避免常见的紫蓝渐变；代码配色同时为两套主题准备，保证对比度。

use eframe::egui::{
    self, Color32, CornerRadius, FontFamily, FontId, Margin, Stroke, TextStyle,
};
use mojuan_core::{Theme, TokenStyle};

pub const R_SM: u8 = 6;
pub const R_MD: u8 = 10;
pub const R_LG: u8 = 14;

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
                bg: c(0x121110),
                panel: c(0x171514),
                elev: c(0x1E1B18),
                edit_bg: c(0x121110),
                read_bg: c(0x181513),
                border: c(0x2C2723),
                border_soft: c(0x221E1B),
                text: c(0xEDE7DC),
                text_dim: c(0x9C9186),
                text_faint: c(0x6B625A),
                accent: c(0xC4614A),
                accent_hover: c(0xD7745C),
                accent_soft: Color32::from_rgba_unmultiplied(196, 97, 74, 30),
                link: c(0xCB9C68),
                code_bg: c(0x1A1714),
                code_border: c(0x2A2521),
                inline_code_bg: c(0x272220),
                quote_bar: c(0x6E4B3E),
                table_head: c(0x1D1A17),
                table_line: c(0x2A2521),
                sel: Color32::from_rgba_unmultiplied(196, 97, 74, 64),
                danger: c(0xD86A5A),
                check: c(0x8FA678),
                tokens: Tokens {
                    plain: c(0xD9D2C7),
                    comment: c(0x6F6A61),
                    keyword: c(0xC9836F),
                    string: c(0x9EB289),
                    number: c(0xCCA468),
                    constant: c(0xCCA468),
                    function: c(0x93B0C6),
                    ty: c(0xC5A97B),
                    tag: c(0xC48A73),
                    attribute: c(0xB99E77),
                    variable: c(0xD6CFC3),
                    operator: c(0xA89B8C),
                    punct: c(0x877E74),
                    invalid: c(0xE0705E),
                },
            },
            Theme::Light => Self {
                theme,
                dark: false,
                bg: c(0xF5F1E8),
                panel: c(0xFAF7F1),
                elev: c(0xFFFDF9),
                edit_bg: c(0xFCFAF5),
                read_bg: c(0xFFFDF8),
                border: c(0xE2D9C9),
                border_soft: c(0xECE4D5),
                text: c(0x2B2621),
                text_dim: c(0x7B7267),
                text_faint: c(0xA79D8E),
                accent: c(0xB24A36),
                accent_hover: c(0xC25A44),
                accent_soft: Color32::from_rgba_unmultiplied(178, 74, 54, 24),
                link: c(0x9A6A38),
                code_bg: c(0xF4EFE3),
                code_border: c(0xE4DACA),
                inline_code_bg: c(0xEDE6D6),
                quote_bar: c(0xC9A98F),
                table_head: c(0xF3EDE0),
                table_line: c(0xE6DDCC),
                sel: Color32::from_rgba_unmultiplied(178, 74, 54, 46),
                danger: c(0xB4442F),
                check: c(0x6B8A55),
                tokens: Tokens {
                    plain: c(0x34302A),
                    comment: c(0x8D8578),
                    keyword: c(0xA5413A),
                    string: c(0x4E7A52),
                    number: c(0x8A6534),
                    constant: c(0x8A6534),
                    function: c(0x3F6A85),
                    ty: c(0x7A5A32),
                    tag: c(0x8A4A3A),
                    attribute: c(0x7A5A32),
                    variable: c(0x3A3630),
                    operator: c(0x6B6259),
                    punct: c(0x8C8578),
                    invalid: c(0xB4442F),
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

        v.widgets.inactive.bg_fill = alpha(self.text, if self.dark { 12 } else { 10 });
        v.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
        v.widgets.inactive.bg_stroke = Stroke::new(1.0, Color32::TRANSPARENT);
        v.widgets.inactive.fg_stroke = Stroke::new(1.0, self.text_dim);
        v.widgets.inactive.corner_radius = CornerRadius::same(R_SM);

        v.widgets.hovered.bg_fill = alpha(self.text, if self.dark { 22 } else { 16 });
        v.widgets.hovered.weak_bg_fill = alpha(self.text, if self.dark { 22 } else { 14 });
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
