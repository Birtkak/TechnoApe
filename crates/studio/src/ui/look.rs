//! The one look of every page (docs/design/studio-tools.md "Look"): dark and calm, one accent for what is
//! chosen, two text sizes, panels on one grid; and the small pieces every page draws with.
use crate::art::{Img, S};
use eframe::egui::{self, Align2, Color32, FontFamily, FontId, Pos2, Rect, Response, RichText, Sense, Stroke, TextStyle, Ui, Vec2};

/// The one accent: what is chosen.
pub const ACCENT: Color32 = Color32::from_rgb(78, 150, 230);
/// Claude's drafts and problems: status colours, never used for what is chosen.
pub const DRAFT: Color32 = Color32::from_rgb(214, 160, 70);
pub const BAD: Color32 = Color32::from_rgb(225, 85, 80);
pub const GOOD: Color32 = Color32::from_rgb(110, 190, 120);
/// A colour per page, for its tab and its inspector's section marks: where you are. Never for what is chosen.
pub const PAGES: [(&str, Color32); 9] = [
    ("Textures", Color32::from_rgb(64, 186, 170)),
    ("Palette", Color32::from_rgb(214, 108, 180)),
    ("Models", Color32::from_rgb(150, 124, 236)),
    ("Sounds", Color32::from_rgb(168, 200, 72)),
    ("Effects", Color32::from_rgb(236, 132, 66)),
    ("Structures", Color32::from_rgb(190, 150, 110)),
    ("Fauna", Color32::from_rgb(232, 104, 128)),
    ("Plants", Color32::from_rgb(120, 196, 96)),
    // an ice cyan: lighter and greener than the accent, bluer than Textures' teal
    ("Maps", Color32::from_rgb(110, 214, 240)),
];
pub fn page_colour(name: &str) -> Color32 {
    PAGES.iter().find(|p| p.0 == name).map_or(ACCENT, |p| p.1)
}
/// What the developer arranges while working, for [`section`] and [`split`]: the frame puts `studio.txt`'s
/// in before a frame and takes it back after.
#[derive(Clone, Default, PartialEq)]
pub struct Layout {
    /// The open page: its name starts every key below, its colour marks the sections.
    pub page: String,
    pub folded: std::collections::BTreeSet<String>,
    pub splits: std::collections::BTreeMap<String, f32>,
    /// Per page, its sections in the developer's order.
    pub order: std::collections::BTreeMap<String, Vec<String>>,
}
fn layout_id() -> egui::Id {
    egui::Id::unique("studio.layout")
}
pub fn set_layout(ctx: &egui::Context, l: Layout) {
    ctx.data_mut(|d| d.insert_temp(layout_id(), l));
}
pub fn layout(ctx: &egui::Context) -> Layout {
    ctx.data(|d| d.get_temp(layout_id())).unwrap_or_default()
}
/// The upper part's height where a page's work area is two parts, one over the other: its share of `total`
/// as dragged ([`split_bar`]), `default` until then.
pub fn split(ui: &Ui, name: &str, default: f32, total: f32) -> f32 {
    let l = layout(ui.ctx());
    let (lo, hi) = crate::shell::Settings::SPLIT;
    (l.splits.get(&format!("{}/{name}", l.page)).copied().unwrap_or(default).clamp(lo, hi) * total).round()
}
/// The bar between the two parts: a drag gives the upper part more or less of `total`, a double click puts
/// it back.
pub fn split_bar(ui: &mut Ui, name: &str, default: f32, total: f32) {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 8.0), Sense::click_and_drag());
    crate::drive::mark(ui.ctx(), format!("split:{name}"), r);
    let hot = resp.hovered() || resp.dragged();
    if hot {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
    let c = if hot { ACCENT } else { Color32::from_gray(70) };
    ui.painter().rect_filled(Rect::from_center_size(r.center(), Vec2::new(if hot { r.width() } else { 48.0 }, 2.0)), 1.0, c);
    let (lo, hi) = crate::shell::Settings::SPLIT;
    let dy = resp.drag_delta().y;
    let back = resp.double_clicked();
    if (dy != 0.0 && total > 1.0) || back {
        ui.ctx().data_mut(|d| {
            let l = d.get_temp_mut_or_default::<Layout>(layout_id());
            let key = format!("{}/{name}", l.page);
            if back {
                l.splits.remove(&key);
            } else {
                let now = l.splits.get(&key).copied().unwrap_or(default).clamp(lo, hi);
                l.splits.insert(key, (now + dy / total).clamp(lo, hi));
            }
        });
    }
    tip(resp, "Drag: more or less room for the part above; a double click puts it back", "");
}
/// The label column of [`row`].
const LABEL: f32 = 124.0;
/// The grid every gap is a step of.
pub const GAP: f32 = 8.0;
/// The two text sizes.
pub const BODY: f32 = 14.0;
/// (one size: a pixel font is sharp at one pixel size only; small text is told by its colour)
pub const SMALL: f32 = BODY;
pub const ROW: f32 = 34.0;

/// The interface is set in the game's own font (data/font.txt as outlines, `crate::font`); a character it
/// does not draw (a degree sign) falls back to egui's own.
pub fn fonts(ctx: &egui::Context) {
    let cells = client::font_cells(include_str!("../../../../data/font.txt")).expect("data/font.txt");
    let glyphs: Vec<(char, [u8; 8])> = (' '..='~').zip(cells).chain(crate::font::MORE).collect();
    let mut f = egui::FontDefinitions::default();
    // (no hinting, no sub-pixel places: both would bend the pixels' squares)
    let tweak = egui::FontTweak { hinting: Some(false), subpixel_binning: Some(false), ..Default::default() };
    f.font_data.insert("game".into(), std::sync::Arc::new(egui::FontData::from_owned(crate::font::ttf(&glyphs)).tweak(tweak)));
    for fam in f.families.values_mut() {
        fam.insert(0, "game".into());
    }
    ctx.set_fonts(f);
}

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
        // a slider: a thin rail lit up to a slim handle
        sp.slider_rail_height = 4.0;
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
        v.slider_trailing_fill = true;
        v.handle_shape = egui::style::HandleShape::Rect { aspect_ratio: 0.4 };
        let w = &mut v.widgets;
        for (st, bg) in [(&mut w.noninteractive, 32), (&mut w.inactive, 48), (&mut w.hovered, 60), (&mut w.active, 70), (&mut w.open, 56)] {
            st.corner_radius = 3.into();
            st.bg_fill = Color32::from_gray(bg);
            st.weak_bg_fill = Color32::from_gray(bg);
            // (a control keeps its size under the pointer: only its colours answer)
            st.expansion = 0.0;
        }
        // a button: a fine edge a step lighter than its face, the accent's when pressed
        w.inactive.bg_stroke = Stroke::new(1.0, Color32::from_gray(66));
        w.open.bg_stroke = Stroke::new(1.0, Color32::from_gray(80));
        w.active.bg_fill = ACCENT.gamma_multiply(0.5);
        w.active.weak_bg_fill = ACCENT.gamma_multiply(0.5);
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
    // `scale` is per 1/16 m: a fine painting's pixels are half as large, its face as large (ADR 0090)
    let scale = scale * S as f32 / img.h as f32;
    for y in 0..img.h {
        for x in 0..img.h {
            let c = img.get(v, x, y);
            let r = Rect::from_min_size(at + Vec2::new(x as f32, y as f32) * scale, Vec2::splat(scale));
            if c == crate::art::EMPTY {
                // an empty pixel: a checkerboard, two squares a side on a large canvas, one a pixel when small
                let grey = |k: usize| Color32::from_gray(if k % 2 == 0 { 78 } else { 118 });
                if scale >= 8.0 {
                    let h = scale / 2.0;
                    for k in 0..4 {
                        p.rect_filled(Rect::from_min_size(r.min + Vec2::new((k % 2) as f32, (k / 2) as f32) * h, Vec2::splat(h)), 0.0, grey(k % 2 + k / 2));
                    }
                } else {
                    p.rect_filled(r, 0.0, grey(x + y));
                }
                continue;
            }
            p.rect_filled(r, 0.0, if dimmed { dim(c) } else { c32(c) });
        }
    }
}

/// What the sections of one panel measured in a frame: which came, how tall each was (title and all), where
/// each title stood, and where the block of placed ones began.
#[derive(Clone, Default)]
struct Measured {
    seen: Vec<String>,
    h: std::collections::BTreeMap<String, f32>,
    bands: std::collections::BTreeMap<String, Rect>,
}
#[derive(Clone, Default)]
struct Flow {
    frame: u64,
    last: Measured,
    now: Measured,
    /// Where this frame's block of placed sections begins, once the first of them came.
    top: Option<f32>,
}
/// The sections shown, in the developer's order: those the order names first, the rest as the page has them.
pub fn display_order(user: Option<&Vec<String>>, seen: &[String]) -> Vec<String> {
    let user = user.map_or(&[][..], |u| &u[..]);
    user.iter().filter(|t| seen.contains(t)).chain(seen.iter().filter(|t| !user.contains(t))).cloned().collect()
}
/// `title` moved to stand before `at` others of the shown ones; titles the order names that are not shown now
/// (another kind of file's) keep their turn after them.
pub fn reorder(user: Option<&Vec<String>>, shown: &[String], title: &str, at: usize) -> Vec<String> {
    let mut v: Vec<String> = shown.iter().filter(|t| *t != title).cloned().collect();
    v.insert(at.min(v.len()), title.to_string());
    v.extend(user.into_iter().flatten().filter(|t| !shown.contains(t)).cloned());
    v
}
/// An open section's room: the panel itself where the page's own order holds, a place of its own where the
/// developer's does. Either way it is drawn into as the panel is; when it ends it notes how tall it was.
pub struct Sec<'a> {
    flow: Option<(&'a mut Ui, f32)>,
    placed: Option<Ui>,
    key: (egui::Id, String),
}
impl std::ops::Deref for Sec<'_> {
    type Target = Ui;
    fn deref(&self) -> &Ui {
        self.placed.as_ref().unwrap_or_else(|| self.flow.as_ref().unwrap().0)
    }
}
impl std::ops::DerefMut for Sec<'_> {
    fn deref_mut(&mut self) -> &mut Ui {
        match &mut self.placed {
            Some(u) => u,
            None => self.flow.as_mut().unwrap().0,
        }
    }
}
impl Drop for Sec<'_> {
    fn drop(&mut self) {
        let h = match (&self.placed, &self.flow) {
            (Some(u), _) => u.min_rect().height(),
            (None, Some((u, start))) => u.cursor().top() - start,
            _ => return,
        };
        measured(self.ctx(), self.key.0, &self.key.1, h);
    }
}
fn measured(ctx: &egui::Context, flow: egui::Id, title: &str, h: f32) {
    let moved = ctx.data_mut(|d| {
        let f = d.get_temp_mut_or_default::<Flow>(flow);
        f.now.h.insert(title.to_string(), h);
        f.last.h.get(title).is_some_and(|was| (was - h).abs() > 0.5)
    });
    // (a placed section stands by last frame's heights: one more frame puts the others right)
    if moved {
        ctx.request_repaint();
    }
}

/// A section's title in the inspector: a band across the panel with the page's colour on its left, so the
/// groups are told apart at a glance (no line between them is needed). A click folds the section shut or
/// opens it; a drag up or down moves it among the others (both kept in studio.txt). None: draw nothing of it.
#[must_use]
pub fn section<'a>(ui: &'a mut Ui, title: &str) -> Option<Sec<'a>> {
    let ctx = ui.ctx().clone();
    let lay = layout(&ctx);
    let fid = egui::Id::unique(("studio.flow", ui.scope_id()));
    let nr = ctx.cumulative_frame_nr();
    let (last, top) = ctx.data_mut(|d| {
        let f = d.get_temp_mut_or_default::<Flow>(fid);
        if f.frame != nr {
            (f.last, f.frame, f.top) = (std::mem::take(&mut f.now), nr, None);
        }
        f.now.seen.push(title.to_string());
        (f.last.clone(), f.top)
    });
    let user = lay.order.get(&lay.page);
    let shown = display_order(user, &last.seen);
    // placed: the developer's order holds here, and last frame measured every section it shows
    let place = user.is_some() && shown.iter().any(|t| t == title) && shown.iter().all(|t| last.h.contains_key(t));
    let start = ui.cursor().top();
    let mut child = None;
    if place {
        // the first placed section of a frame takes the room of them all; each then stands at its turn in it
        let top = top.unwrap_or_else(|| {
            let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), shown.iter().map(|t| last.h[t]).sum()), Sense::hover());
            ctx.data_mut(|d| d.get_temp_mut_or_default::<Flow>(fid).top = Some(r.top()));
            r.top()
        });
        let y = top + shown.iter().take_while(|t| *t != title).map(|t| last.h[t]).sum::<f32>();
        let r = Rect::from_min_size(Pos2::new(ui.max_rect().left(), y), Vec2::new(ui.available_width(), last.h[title]));
        child = Some(ui.new_child(egui::UiBuilder::new().id_salt(("section", title)).max_rect(r).layout(egui::Layout::top_down(egui::Align::Min))));
    }
    let key = format!("{}/{title}", lay.page);
    let (r, resp) = band(child.as_mut().unwrap_or(ui), title, Sense::click_and_drag());
    crate::drive::mark(&ctx, format!("section:{title}"), r);
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Flow>(fid).now.bands.insert(title.to_string(), r));
    if resp.clicked() {
        ctx.data_mut(|d| {
            let f = &mut d.get_temp_mut_or_default::<Layout>(layout_id()).folded;
            if !f.remove(&key) {
                f.insert(key.clone());
            }
        });
    }
    // a drag: a line where it would go; let go, and it stands there
    if let (true, Some(p)) = (resp.dragged() || resp.drag_stopped(), ctx.pointer_interact_pos()) {
        let others: Vec<&String> = shown.iter().filter(|t| *t != title && last.bands.contains_key(*t)).collect();
        let at = others.iter().filter(|t| last.bands[**t].center().y < p.y).count();
        let y = others.get(at).map(|t| last.bands[*t].top() - GAP * 0.4).or(others.last().map(|t| last.bands[*t].top() + last.h.get(*t).copied().unwrap_or(24.0))).unwrap_or(r.top());
        ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("studio.drop"))).hline(r.x_range(), y, Stroke::new(2.0, ACCENT));
        ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        if resp.drag_stopped() && (p.y - r.center().y).abs() > r.height() {
            let new = reorder(user, &shown, title, at);
            ctx.data_mut(|d| d.get_temp_mut_or_default::<Layout>(layout_id()).order.insert(lay.page.clone(), new));
        }
    }
    let open = !layout(&ctx).folded.contains(&key);
    // the mark on the right: down when open, sideways when shut
    let (c, s) = (r.right_center() - Vec2::new(14.0, 0.0), 4.0);
    let pts = if open { vec![c + Vec2::new(-s, -s * 0.6), c + Vec2::new(s, -s * 0.6), c + Vec2::new(0.0, s * 0.8)] } else { vec![c + Vec2::new(-s * 0.6, -s), c + Vec2::new(s * 0.8, 0.0), c + Vec2::new(-s * 0.6, s)] };
    child.as_ref().unwrap_or(ui).painter().add(egui::Shape::convex_polygon(pts, Color32::from_gray(if resp.hovered() { 230 } else { 150 }), Stroke::NONE));
    // under the pointer: six dots, the grip a drag takes
    if resp.hovered() {
        for (i, j) in (0..3).flat_map(|i| [(i, 0), (i, 1)]) {
            child.as_ref().unwrap_or(ui).painter().circle_filled(c + Vec2::new(-18.0 - 4.0 * j as f32, -4.0 + 4.0 * i as f32), 1.0, Color32::from_gray(190));
        }
    }
    tip(resp, "Click folds this section or opens it; a drag moves it up or down", "");
    let sec = Sec { key: (fid, title.to_string()), flow: if child.is_none() { Some((ui, start)) } else { None }, placed: child };
    // (a folded one is measured at once, as it ends here)
    open.then_some(sec)
}
impl Sec<'_> {
    fn ctx(&self) -> &egui::Context {
        std::ops::Deref::deref(self).ctx()
    }
}
/// The band alone: a title that does not fold (the key sheet's).
pub fn band(ui: &mut Ui, title: &str, sense: Sense) -> (Rect, Response) {
    ui.add_space(GAP * 0.75);
    let c = page_colour(&layout(ui.ctx()).page);
    let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 24.0), sense);
    let p = ui.painter();
    p.rect_filled(r, 3.0, c.gamma_multiply(if resp.hovered() { 0.22 } else { 0.13 }));
    p.rect_filled(Rect::from_min_size(r.min, Vec2::new(3.0, r.height())), 1.0, c);
    p.text(r.left_center() + Vec2::new(GAP + 3.0, 0.0), Align2::LEFT_CENTER, title, FontId::proportional(BODY), ui.visuals().strong_text_color());
    (r, resp)
}
/// A label and its control on one line, every label in one column so the controls line up; the label goes
/// over the control when the panel is narrow (a large interface scale). A slider in it fills the line.
/// A slider's number box in a [`row`] ("1.20 m/s" fits).
pub const VALUE: f32 = 80.0;
pub fn row<R>(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    if ui.available_width() < 250.0 {
        ui.label(RichText::new(label).small());
        ui.spacing_mut().slider_width = (ui.available_width() - 100.0).max(40.0);
        return add(ui);
    }
    ui.horizontal(|ui| {
        self::label(ui, label);
        // (chips that do not fit go on under the controls, not under the label)
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            // every slider's number box as wide as the next one's, so their right edges are one line
            ui.spacing_mut().interact_size.x = VALUE;
            ui.spacing_mut().slider_width = (ui.available_width() - VALUE - 6.0).max(40.0);
            add(ui)
        })
        .inner
    })
    .inner
}
/// A label as wide as [`row`]'s, for a grid's first cell.
pub fn label(ui: &mut Ui, text: &str) {
    let (r, _) = ui.allocate_exact_size(Vec2::new(LABEL, ui.spacing().interact_size.y), Sense::hover());
    ui.painter().text(r.left_center(), Align2::LEFT_CENTER, text, FontId::proportional(SMALL), ui.visuals().text_color());
}
/// A title in the work area: strong text, no band (a band is the inspector's).
pub fn heading(ui: &mut Ui, title: &str) {
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
        say(&r.ctx, &text);
    }
    r.on_hover_text(text)
}
/// The status line's left this frame, as a control's tip puts it (a wire refused while it is dragged).
pub fn say(ctx: &egui::Context, text: &str) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique("studio.help"), text.to_string()));
}
/// The help the pointer stands on this frame, taken once.
pub fn take_help(ctx: &egui::Context) -> Option<String> {
    ctx.data_mut(|d| d.remove_temp::<String>(egui::Id::unique("studio.help")))
}

/// A tool's button: its name, and its key weak beside it; `w` wide at least, so tools stand in columns.
pub fn tool(ui: &mut Ui, on: bool, name: &str, key: &str, w: f32) -> Response {
    static ICONS: std::sync::OnceLock<std::collections::BTreeMap<String, Vec<String>>> = std::sync::OnceLock::new();
    let icon = ICONS.get_or_init(|| icons(include_str!("../icons.txt"))).get(&name.to_lowercase());
    let colour = if on { Color32::WHITE } else { Color32::from_gray(215) };
    let mut job = egui::text::LayoutJob::default();
    // (room for the icon before the name)
    job.append(name, if icon.is_some() { ICON + 5.0 } else { 0.0 }, egui::TextFormat::simple(FontId::proportional(BODY), colour));
    job.append(key, 6.0, egui::TextFormat::simple(FontId::proportional(SMALL), Color32::from_gray(if on { 200 } else { 130 })));
    let wide = ui.fonts_mut(|f| f.layout_job(job.clone())).size().x;
    let r = ui.add(egui::Button::selectable(on, egui::WidgetText::from(job)).min_size(Vec2::new(w, 26.0)));
    if let Some(rows) = icon {
        // at the left of the button's text (the button centres it), a cell the font's pixel
        let px = ICON / 8.0;
        let k = ui.ctx().pixels_per_point();
        let at = Pos2::new(((r.rect.center().x - wide / 2.0) * k).round() / k, ((r.rect.center().y - rows.len() as f32 * px / 2.0) * k).round() / k);
        for (j, row) in rows.iter().enumerate() {
            for (i, _) in row.chars().enumerate().filter(|(_, c)| *c == '#') {
                ui.painter().rect_filled(Rect::from_min_size(at + Vec2::new(i as f32, j as f32) * px, Vec2::splat(px)), 0.0, colour);
            }
        }
    }
    r
}
/// An icon's side in points: 8 cells of the font's pixel (1.5 points at size 14).
const ICON: f32 = 12.0;
/// The icons of `icons.txt` by name: each its rows, # lit. A row wider than 8 or a ninth row is left out.
pub fn icons(text: &str) -> std::collections::BTreeMap<String, Vec<String>> {
    let mut all = std::collections::BTreeMap::<String, Vec<String>>::new();
    let mut at = None;
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with("//")) {
        if line.chars().all(|c| c == '#' || c == '.') {
            if let Some(rows) = at.as_ref().and_then(|n| all.get_mut(n)).filter(|r: &&mut Vec<String>| r.len() < 8 && line.len() <= 8) {
                rows.push(line.to_string());
            }
        } else {
            at = Some(line.to_string());
            all.insert(line.to_string(), vec![]);
        }
    }
    all
}
/// An option that is on or off beside the tools: a chip, so it is not taken for a tool.
pub fn toggle(ui: &mut Ui, on: &mut bool, text: &str) -> Response {
    let mut r = chip(ui, *on, text);
    if r.clicked() {
        *on = !*on;
        r.mark_changed();
    }
    r
}
/// "dirt: dig" is the row "dig" under the heading "dirt"; "oak (grown)" is the row "oak" with the tag "grown".
pub fn split_label(label: &str) -> (Option<&str>, &str, Option<&str>) {
    let (group, rest) = label.split_once(": ").map_or((None, label), |(g, r)| (Some(g), r));
    match rest.strip_suffix(')').and_then(|r| r.rsplit_once(" (")) {
        Some((name, tag)) => (group, name, Some(tag)),
        None => (group, rest, None),
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn sections_stand_in_the_developers_order_and_a_moved_one_keeps_the_rest() {
        let v = |l: &[&str]| l.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let seen = v(&["Body", "Parts", "Gait", "Sounds"]);
        // no order: the page's own; an order names some, the rest follow as the page has them
        assert_eq!(super::display_order(None, &seen), seen);
        assert_eq!(super::display_order(Some(&v(&["Gait", "Recipe", "Body"])), &seen), v(&["Gait", "Body", "Parts", "Sounds"]));
        // Sounds dragged to before the second of the others; a title of another kind of file keeps its turn
        assert_eq!(super::reorder(Some(&v(&["Gait", "Recipe", "Body"])), &v(&["Gait", "Body", "Parts", "Sounds"]), "Sounds", 1), v(&["Gait", "Sounds", "Body", "Parts", "Recipe"]));
        assert_eq!(super::reorder(None, &seen, "Body", 99), v(&["Parts", "Gait", "Sounds", "Body"]));
    }
    #[test]
    fn every_tool_of_both_pages_has_an_icon_that_fits() {
        let all = super::icons(include_str!("../icons.txt"));
        assert!(all.values().all(|rows| !rows.is_empty() && rows.len() <= 8 && rows.iter().all(|r| r.len() <= 8)), "{all:?}");
        for name in crate::ui::models::TOOLS.iter().map(|t| t.2).chain(crate::ui::textures::TOOLS.iter().map(|t| t.2)) {
            assert!(all.contains_key(&name.to_lowercase()), "no icon for {name}");
        }
        // a row too wide and a ninth row are left out; a name with no rows is no icon to draw
        let odd = super::icons("a\n#########\n#\nb\n#\n#\n#\n#\n#\n#\n#\n#\n#\n");
        assert_eq!((odd["a"].len(), odd["b"].len()), (1, 8));
    }

    #[test]
    fn a_files_label_splits_into_heading_name_and_tag() {
        use super::split_label as s;
        assert_eq!(s("dirt: dig"), (Some("dirt"), "dig", None));
        assert_eq!(s("oak (grown)"), (None, "oak", Some("grown")));
        assert_eq!(s("creatures.toml (refused)"), (None, "creatures.toml", Some("refused")));
        assert_eq!(s("grass"), (None, "grass", None));
    }
}

/// A small chip: selected shows the accent.
pub fn chip(ui: &mut Ui, on: bool, text: &str) -> Response {
    let t = RichText::new(text).small();
    ui.add(egui::Button::selectable(on, if on { t.color(Color32::WHITE) } else { t }).corner_radius(10.0))
}
