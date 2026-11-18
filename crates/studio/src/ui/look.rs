//! The one look of every page (docs/design/studio-tools.md "Look", ADR draft-studio-look): neutral dark greys,
//! one blue for the main action and for focus, a clean sans in two weights, quiet controls; and the small
//! pieces every page draws with.
use crate::art::{Img, S};
use eframe::egui::{self, Align2, Color32, FontFamily, FontId, Pos2, Rect, Response, RichText, Sense, Stroke, TextStyle, Ui, Vec2};

/// The colour tokens: every grey of the frame is one of these.
const fn hex(c: u32) -> Color32 {
    Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}
/// The work area, and the wells of fields.
pub const BG: Color32 = hex(0x1e1e1e);
/// Panels, menus, cards.
pub const PANEL: Color32 = hex(0x2a2a2a);
/// Raised controls and hovered rows.
pub const RAISED: Color32 = hex(0x353535);
/// Pressed or chosen.
pub const PRESSED: Color32 = hex(0x434343);
/// Hairlines between parts.
pub const LINE: Color32 = hex(0x3d3d3d);
/// A field's border, a slider's rail.
pub const FIELD: Color32 = hex(0x525252);
pub const TEXT: Color32 = hex(0xe6e6e6);
pub const TEXT2: Color32 = hex(0xb0b0b0);
pub const DISABLED: Color32 = hex(0x6e6e6e);
/// The one blue: the page's main action and focus.
pub const ACCENT: Color32 = hex(0x2680eb);
pub const ACCENT_HOVER: Color32 = hex(0x378ef0);
/// Status colours, never for what is chosen (a step lighter than Spectrum's fills, so text in them reads at
/// 4.5:1 on a panel).
pub const GOOD: Color32 = hex(0x33ab84);
/// Claude's drafts, unsaved work: notice.
pub const DRAFT: Color32 = hex(0xe68619);
pub const BAD: Color32 = hex(0xf0626a);
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
    let c = if hot { ACCENT } else { FIELD };
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
pub const ROW: f32 = 34.0;

/// The interface font: the clean sans (false) or the game's pixel font (true, the setting "Interface font").
static GAME_FONT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub fn game_font() -> bool {
    GAME_FONT.load(std::sync::atomic::Ordering::Relaxed)
}
/// The two text sizes: body and small (the game font has one: it is sharp at one pixel size only, and small
/// text is told by its colour).
pub fn body() -> f32 {
    if game_font() { 14.0 } else { 13.0 }
}
pub fn small() -> f32 {
    if game_font() { 14.0 } else { 11.5 }
}
/// The semibold face: titles, tabs, the main buttons.
pub fn strong(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("strong".into()))
}

/// The fonts: the clean sans read from the system (Segoe UI, regular and semibold; egui's own where it is
/// missing), or the game's own font (data/font.txt as outlines, `crate::font`), whose missing characters (a
/// degree sign) fall back to egui's.
pub fn fonts(ctx: &egui::Context, game: bool) {
    GAME_FONT.store(game, std::sync::atomic::Ordering::Relaxed);
    let mut f = egui::FontDefinitions::default();
    let base = f.families[&FontFamily::Proportional].clone();
    let mut strong = base.clone();
    if game {
        let cells = client::font_cells(include_str!("../../../../data/font.txt")).expect("data/font.txt");
        let glyphs: Vec<(char, [u8; 8])> = (' '..='~').zip(cells).chain(crate::font::MORE).collect();
        // (no hinting, no sub-pixel places: both would bend the pixels' squares)
        let tweak = egui::FontTweak { hinting: Some(false), subpixel_binning: Some(false), ..Default::default() };
        f.font_data.insert("game".into(), std::sync::Arc::new(egui::FontData::from_owned(crate::font::ttf(&glyphs)).tweak(tweak)));
        for fam in f.families.values_mut() {
            fam.insert(0, "game".into());
        }
        strong.insert(0, "game".into());
    } else {
        let read = |n: &str| std::fs::read(format!("C:/Windows/Fonts/{n}")).ok();
        if let Some(b) = read("segoeui.ttf") {
            f.font_data.insert("clean".into(), std::sync::Arc::new(egui::FontData::from_owned(b)));
            f.families.get_mut(&FontFamily::Proportional).unwrap().insert(0, "clean".into());
            strong.insert(0, "clean".into());
        }
        if let Some(b) = read("seguisb.ttf") {
            f.font_data.insert("clean semibold".into(), std::sync::Arc::new(egui::FontData::from_owned(b)));
            strong.insert(0, "clean semibold".into());
        }
    }
    f.families.insert(FontFamily::Name("strong".into()), strong);
    ctx.set_fonts(f);
}

/// Plain buttons quiet (a fill only under the pointer), number boxes and drop-downs framed as fields, a chosen
/// button filled `PRESSED` in white, the main action's pill (`primary`) blue.
struct Theme;
impl egui::theme::StyleProvider<egui::widget_style::ButtonStyle> for Theme {
    fn style(&mut self, a: &egui::widget_style::StyleArgs<'_>) -> egui::widget_style::ButtonStyle {
        use egui::widget_style::WidgetState as W;
        let mut s: egui::widget_style::ButtonStyle = egui::theme::StyleProvider::style(&mut egui::theme::DefaultStyle, a);
        let f = &mut s.atom_layout.frame;
        let field = a.classes.iter().any(|c| c.as_str() == egui::DragValue::CLASS.as_str()) || a.classes.iter().any(|c| c.as_str() == egui::ComboBox::CLASS.as_str());
        if a.classes.iter().any(|c| c.as_str() == egui::Button::CLASS_SELECTED.as_str()) {
            f.fill = PRESSED;
            s.atom_layout.text_style.color = Color32::WHITE;
        } else if field {
            f.fill = BG;
            f.stroke = Stroke::new(1.0, if a.state == W::Inactive { FIELD } else { DISABLED });
        }
        s
    }
}
impl egui::theme::StyleProvider<egui::widget_style::TextEditStyle> for Theme {
    fn style(&mut self, a: &egui::widget_style::StyleArgs<'_>) -> egui::widget_style::TextEditStyle {
        use egui::widget_style::WidgetState as W;
        let mut s: egui::widget_style::TextEditStyle = egui::theme::StyleProvider::style(&mut egui::theme::DefaultStyle, a);
        let f = &mut s.atom_layout.frame;
        if !a.classes.iter().any(|c| c.as_str() == egui::TextEdit::CLASS_READ_ONLY.as_str()) {
            // a 1 px border; focused, a 2 px ring in the accent
            f.stroke = match a.state {
                W::Active => Stroke::new(2.0, ACCENT),
                W::Hovered => Stroke::new(1.0, DISABLED),
                _ => Stroke::new(1.0, FIELD),
            };
        }
        s
    }
}

pub fn apply(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Dark);
    ctx.add_widget_theme::<egui::widget_style::ButtonStyle>(Theme);
    ctx.add_widget_theme::<egui::widget_style::TextEditStyle>(Theme);
    let (body, small) = (body(), small());
    ctx.style_mut_of(egui::Theme::Dark, |s| {
        s.text_styles = [
            (TextStyle::Small, FontId::new(small, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(body, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(body, FontFamily::Proportional)),
            (TextStyle::Heading, strong(body)),
            (TextStyle::Monospace, FontId::new(small, FontFamily::Monospace)),
        ]
        .into();
        let sp = &mut s.spacing;
        sp.item_spacing = Vec2::new(GAP, GAP * 0.75);
        sp.button_padding = Vec2::new(GAP + 2.0, GAP * 0.5);
        // (a slider's handle is 0.8 of this high: 16 points)
        sp.interact_size = Vec2::new(40.0, 20.0);
        sp.window_margin = egui::Margin::same(GAP as i8 * 2);
        sp.menu_margin = egui::Margin::same(GAP as i8);
        sp.slider_width = 150.0;
        // a slider: a 2 point rail, lit up to a round handle
        sp.slider_rail_height = 2.0;
        sp.text_edit_width = 200.0;
        sp.indent = GAP * 2.0;
        s.interaction.tooltip_delay = 0.35;
        let v = &mut s.visuals;
        v.panel_fill = PANEL;
        v.window_fill = PANEL;
        v.extreme_bg_color = BG;
        v.faint_bg_color = BG;
        v.code_bg_color = BG;
        v.window_stroke = Stroke::new(1.0, LINE);
        v.window_corner_radius = 8.into();
        v.menu_corner_radius = 8.into();
        // a soft shadow under menus, tips and boxes
        v.window_shadow = egui::Shadow { offset: [0, 6], blur: 20, spread: 0, color: Color32::from_black_alpha(120) };
        v.popup_shadow = egui::Shadow { offset: [0, 4], blur: 14, spread: 0, color: Color32::from_black_alpha(100) };
        // (the slider's lit part and selected text; a chosen button is `PRESSED`, by the theme above)
        v.selection.bg_fill = ACCENT;
        v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
        v.hyperlink_color = ACCENT_HOVER;
        v.weak_text_color = Some(TEXT2);
        v.override_text_color = None;
        v.slider_trailing_fill = true;
        v.handle_shape = egui::style::HandleShape::Circle;
        let w = &mut v.widgets;
        for (st, bg, weak, fg) in [
            (&mut w.noninteractive, PANEL, PANEL, TEXT),
            (&mut w.inactive, FIELD, Color32::TRANSPARENT, TEXT),
            (&mut w.hovered, DISABLED, RAISED, Color32::WHITE),
            (&mut w.active, TEXT2, PRESSED, Color32::WHITE),
            (&mut w.open, DISABLED, RAISED, Color32::WHITE),
        ] {
            st.corner_radius = 4.into();
            st.bg_fill = bg;
            st.weak_bg_fill = weak;
            // (2 points: a check's tick and a slider handle's ring; text takes only the colour)
            st.fg_stroke = Stroke::new(2.0, fg);
            // an edge only where a state draws one, the same width in all so nothing moves
            st.bg_stroke = Stroke::new(1.0, Color32::TRANSPARENT);
            // (a control keeps its size under the pointer: only its colours answer)
            st.expansion = 0.0;
        }
        // separators and hairlines
        w.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
        w.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    });
}

/// The page's main action: a filled blue pill (Use, Save with work to save, Try in game, Import). One a page.
pub fn primary(text: &str) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text.to_string()).font(strong(body())).color(Color32::WHITE)).fill(ACCENT).corner_radius(14.0)
}
/// A row of chips as one segmented group: a well with the chosen segment raised.
pub fn segmented<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new().fill(BG).stroke(Stroke::new(1.0, LINE)).corner_radius(6.0).inner_margin(2.0).show(ui, |ui| ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(2.0, 2.0);
        add(ui)
    }).inner).inner
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
    // the chevron on the right: down when open, sideways when shut
    let (c, s) = (r.right_center() - Vec2::new(14.0, 0.0), 3.5);
    let pts = if open { vec![c + Vec2::new(-s, -s * 0.5), c + Vec2::new(0.0, s * 0.5), c + Vec2::new(s, -s * 0.5)] } else { vec![c + Vec2::new(-s * 0.5, -s), c + Vec2::new(s * 0.5, 0.0), c + Vec2::new(-s * 0.5, s)] };
    child.as_ref().unwrap_or(ui).painter().add(egui::Shape::line(pts, Stroke::new(1.5, if resp.hovered() { TEXT } else { TEXT2 })));
    // under the pointer: six dots, the grip a drag takes
    if resp.hovered() {
        for (i, j) in (0..3).flat_map(|i| [(i, 0), (i, 1)]) {
            child.as_ref().unwrap_or(ui).painter().circle_filled(c + Vec2::new(-18.0 - 4.0 * j as f32, -4.0 + 4.0 * i as f32), 1.0, TEXT2);
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
/// The band alone: a title that does not fold (the key sheet's). Flat: a hairline above it, a 2 point bar in
/// the page's colour at its left, the title in the semibold face.
pub fn band(ui: &mut Ui, title: &str, sense: Sense) -> (Rect, Response) {
    ui.add_space(GAP * 0.5);
    let c = page_colour(&layout(ui.ctx()).page);
    let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), sense);
    let p = ui.painter();
    p.hline(r.x_range(), r.top(), Stroke::new(1.0, LINE));
    if resp.hovered() {
        p.rect_filled(r.shrink2(Vec2::new(0.0, 1.0)), 4.0, RAISED);
    }
    p.rect_filled(Rect::from_min_size(r.min + Vec2::new(0.0, 7.0), Vec2::new(2.0, r.height() - 13.0)), 1.0, c);
    p.text(r.left_center() + Vec2::new(GAP + 2.0, 0.5), Align2::LEFT_CENTER, title, strong(body()), Color32::WHITE);
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
            // (chips side by side read as one segmented group)
            ui.spacing_mut().item_spacing.x = 2.0;
            // every slider's number box as wide as the next one's, so their right edges are one line
            ui.spacing_mut().interact_size.x = VALUE;
            ui.spacing_mut().slider_width = (ui.available_width() - VALUE - 6.0).max(40.0);
            add(ui)
        })
        .inner
    })
    .inner
}
/// A label as wide as [`row`]'s, for a grid's first cell; one too long ends in "…" (its whole text in the tip).
pub fn label(ui: &mut Ui, text: &str) {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(LABEL, ui.spacing().interact_size.y), Sense::hover());
    let mut job = egui::text::LayoutJob::simple_singleline(text.into(), FontId::proportional(body()), TEXT2);
    job.wrap = egui::text::TextWrapping::truncate_at_width(LABEL - 4.0);
    let g = ui.painter().layout_job(job);
    let cut = g.elided;
    ui.painter().galley(Pos2::new(r.left(), r.center().y - g.size().y / 2.0), g, TEXT2);
    if cut {
        resp.on_hover_text(text);
    }
}
/// A title in the work area: the semibold face, no band (a band is the inspector's).
pub fn heading(ui: &mut Ui, title: &str) {
    ui.add_space(GAP * 0.5);
    ui.label(RichText::new(title).font(strong(body())).color(Color32::WHITE));
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
    if key.is_empty() {
        return r.on_hover_text(text);
    }
    // the key in a small keycap after the help
    let (help, key) = (help.to_string(), key.to_string());
    r.on_hover_ui(move |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label(help.clone());
            keycap(ui, &key);
        });
    })
}
/// A key as a small keycap: "Ctrl+S" on a raised rounded rectangle.
pub fn keycap(ui: &mut Ui, key: &str) -> Response {
    egui::Frame::new().fill(RAISED).stroke(Stroke::new(1.0, FIELD)).corner_radius(4.0).inner_margin(egui::Margin::symmetric(5, 1)).show(ui, |ui| ui.label(RichText::new(key).size(small()).color(TEXT))).inner
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
    let colour = if on { Color32::WHITE } else { TEXT };
    let mut job = egui::text::LayoutJob::default();
    // (room for the icon before the name)
    job.append(name, if icon.is_some() { ICON + 5.0 } else { 0.0 }, egui::TextFormat::simple(FontId::proportional(body()), colour));
    job.append(key, 6.0, egui::TextFormat::simple(FontId::proportional(small()), if on { TEXT2 } else { DISABLED }));
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

/// A chip: one segment of a group (side by side in a [`row`], or in [`segmented`]); the chosen one raised in
/// white.
pub fn chip(ui: &mut Ui, on: bool, text: &str) -> Response {
    let t = RichText::new(text).size(body());
    ui.add(egui::Button::new(if on { t.color(Color32::WHITE) } else { t.color(TEXT2) }).fill(if on { PRESSED } else { BG }).corner_radius(4.0).min_size(Vec2::new(0.0, 24.0)))
}
