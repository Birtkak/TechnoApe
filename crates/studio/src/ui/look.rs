//! The one look of every page (docs/design/studio-tools.md "Look"): dark and calm, one accent for what is
//! chosen, two text sizes, panels on one grid; and the small pieces every page draws with.
use crate::art::{Img, S};
use eframe::egui::{self, Color32, FontFamily, FontId, Pos2, Rect, Response, RichText, Stroke, TextStyle, Ui, Vec2};

/// The one accent: what is chosen.
pub const ACCENT: Color32 = Color32::from_rgb(78, 150, 230);
/// Claude's drafts and problems: status colours, never used for what is chosen.
pub const DRAFT: Color32 = Color32::from_rgb(214, 160, 70);
pub const BAD: Color32 = Color32::from_rgb(225, 85, 80);
pub const GOOD: Color32 = Color32::from_rgb(110, 190, 120);
/// The grid every gap is a step of.
pub const GAP: f32 = 8.0;
/// The two text sizes.
pub const BODY: f32 = 14.0;
pub const SMALL: f32 = 12.0;
pub const ROW: f32 = 34.0;

pub fn apply(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Dark);
    ctx.style_mut_of(egui::Theme::Dark, |s| {
        s.text_styles = [
            (TextStyle::Small, FontId::new(SMALL, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(BODY, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(BODY, FontFamily::Proportional)),
            (TextStyle::Heading, FontId::new(BODY, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(SMALL, FontFamily::Monospace)),
        ]
        .into();
        let sp = &mut s.spacing;
        sp.item_spacing = Vec2::new(GAP, GAP * 0.75);
        sp.button_padding = Vec2::new(GAP, GAP * 0.5);
        sp.interact_size = Vec2::new(40.0, 26.0);
        sp.window_margin = egui::Margin::same(GAP as i8 * 2);
        sp.menu_margin = egui::Margin::same(GAP as i8);
        sp.slider_width = 150.0;
        sp.text_edit_width = 200.0;
        sp.indent = GAP * 2.0;
        s.interaction.tooltip_delay = 0.35;
        let v = &mut s.visuals;
        v.panel_fill = Color32::from_rgb(32, 34, 38);
        v.window_fill = Color32::from_rgb(40, 42, 47);
        v.extreme_bg_color = Color32::from_rgb(22, 23, 26);
        v.faint_bg_color = Color32::from_rgb(38, 40, 45);
        v.window_stroke = Stroke::new(1.0, Color32::from_rgb(58, 61, 68));
        v.window_corner_radius = 6.into();
        v.menu_corner_radius = 6.into();
        v.selection.bg_fill = ACCENT.gamma_multiply(0.55);
        v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
        v.hyperlink_color = ACCENT;
        v.override_text_color = None;
        let w = &mut v.widgets;
        for (st, bg) in [(&mut w.noninteractive, 32), (&mut w.inactive, 50), (&mut w.hovered, 62), (&mut w.active, 72), (&mut w.open, 56)] {
            st.corner_radius = 4.into();
            st.bg_fill = Color32::from_gray(bg);
            st.weak_bg_fill = Color32::from_gray(bg);
        }
        w.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(52, 55, 61));
        w.noninteractive.fg_stroke = Stroke::new(1.0, Color32::from_gray(205));
        w.inactive.fg_stroke = Stroke::new(1.0, Color32::from_gray(215));
        w.hovered.bg_stroke = Stroke::new(1.0, ACCENT.gamma_multiply(0.7));
        w.active.bg_stroke = Stroke::new(1.0, ACCENT);
    });
}

pub fn c32(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}
/// A colour greyed and darkened: a face that only falls back to another file, an unpainted block.
pub fn dim(c: [u8; 3]) -> Color32 {
    let g = (c[0] as u16 + c[1] as u16 + c[2] as u16) / 3;
    let m = |v: u8| ((v as u16 + g * 2) / 3 / 2 + 40) as u8;
    Color32::from_rgb(m(c[0]), m(c[1]), m(c[2]))
}

/// One variant of a texture, one rectangle per pixel (ADR 0046: no GPU textures, a shot draws what the window does).
pub fn draw(p: &egui::Painter, img: &Img, v: usize, at: Pos2, scale: f32, dimmed: bool) {
    for y in 0..S {
        for x in 0..S {
            let c = img.get(v, x, y);
            let r = Rect::from_min_size(at + Vec2::new(x as f32, y as f32) * scale, Vec2::splat(scale));
            p.rect_filled(r, 0.0, if dimmed { dim(c) } else { c32(c) });
        }
    }
}

/// A section's title in the inspector: the body size, strong, with the grid's gap above it.
pub fn section(ui: &mut Ui, title: &str) {
    ui.add_space(GAP * 0.5);
    ui.label(RichText::new(title).strong());
}
pub fn weak(ui: &mut Ui, text: impl Into<String>) -> Response {
    ui.label(RichText::new(text.into()).small().weak())
}

/// What a control does and its key: the tooltip, and the status line's left while the pointer is on it.
pub fn tip(r: Response, help: &str, key: &str) -> Response {
    let text = if key.is_empty() { help.to_string() } else { format!("{help}   [{key}]") };
    if r.hovered() {
        r.ctx.data_mut(|d| d.insert_temp(egui::Id::unique("studio.help"), text.clone()));
    }
    r.on_hover_text(text)
}
/// The help the pointer stands on this frame, taken once.
pub fn take_help(ctx: &egui::Context) -> Option<String> {
    ctx.data_mut(|d| d.remove_temp::<String>(egui::Id::unique("studio.help")))
}

/// A small chip: selected shows the accent.
pub fn chip(ui: &mut Ui, on: bool, text: &str) -> Response {
    let t = RichText::new(text).small();
    ui.add(egui::Button::selectable(on, if on { t.color(Color32::WHITE) } else { t }).corner_radius(10.0))
}
