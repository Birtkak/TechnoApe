//! The variations wall as one widget (docs/design/studio-tools.md "The variations wall"): a page hands it a
//! [`Wall`] and a way to draw one item; it shows one action bar (Use and Cancel where the page's pick waits,
//! "More like this", "New batch", "Back", how different) over a grid of large pictures, and says what was
//! clicked. The page then takes the item, or refills the wall with its own generator.
use super::look::{self, GAP};
use crate::wall::Wall;
use eframe::egui::{self, Color32, FontId, Key, Modifiers, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Ui, Vec2};

#[derive(PartialEq, Debug)]
pub enum Act {
    None,
    /// Take this item into the editor (a click: the pick, which the preview follows).
    Take(usize),
    /// A new wall around this item.
    More(usize),
    Back,
    /// Other candidates around the same thing.
    Shuffle,
    /// The spread slider moved: make the wall again.
    Spread,
    /// Hear this item (a page that plays its items).
    Play(usize),
    /// Keep the pick (the Use pill, Enter, a double click) on a page whose pick waits for it.
    Use,
    /// Drop the pick (Cancel, Escape) on a page whose pick waits for it.
    Cancel,
}

/// `chosen`: the item that is in the editor now, or picked (ringed). `cell`: the least size of one picture;
/// the cells grow to share the width, up to half as large again.
pub fn show<T: Clone>(ui: &mut Ui, wall: &mut Wall<T>, chosen: Option<usize>, cell: Vec2, draw: impl Fn(&egui::Painter, Rect, &T)) -> Act {
    show_full(ui, wall, chosen, cell, false, None, |p, r, _, t| draw(p, r, t))
}

/// `show` with a play button on each item when `play`, and the item's index given to `draw`.
pub fn show_with<T: Clone>(ui: &mut Ui, wall: &mut Wall<T>, chosen: Option<usize>, cell: Vec2, play: bool, draw: impl Fn(&egui::Painter, Rect, usize, &T)) -> Act {
    show_full(ui, wall, chosen, cell, play, None, draw)
}

/// Columns and the side of a cell for `width`: as large as `most` allows, never under `least` while one
/// fits, the gaps between. The cells fill the width but for less than one cell.
pub fn fit(width: f32, gap: f32, least: f32, most: f32) -> (usize, f32) {
    let width = width.max(1.0);
    let room = ((width + gap) / (least + gap)).floor().max(1.0);
    let cols = ((width + gap) / (most + gap)).ceil().clamp(1.0, room);
    (cols as usize, ((width + gap) / cols - gap).min(most).min(width).floor().max(1.0))
}

/// Whether a wall was on screen last frame: its keys (M, Enter, Escape) are then the wall's, not the page's.
pub fn shown(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<u64>(egui::Id::unique("wall.frame"))).is_some_and(|f| f + 1 >= ctx.cumulative_frame_nr())
}

/// `show_with` where `uses` is `Some(picked)` on a page whose pick waits for Use: the bar then leads with
/// Use (on while something is picked) and Cancel.
pub fn show_full<T: Clone>(ui: &mut Ui, wall: &mut Wall<T>, chosen: Option<usize>, cell: Vec2, play: bool, uses: Option<bool>, draw: impl Fn(&egui::Painter, Rect, usize, &T)) -> Act {
    let mut act = Act::None;
    let ctx = ui.ctx().clone();
    let frame = ctx.cumulative_frame_nr();
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique("wall.frame"), frame));
    // the one bar
    ui.horizontal_wrapped(|ui| {
        if let Some(picked) = uses {
            let r = ui.add_enabled(picked, look::primary("Use"));
            crate::drive::mark(ui.ctx(), "btn:wall.use", r.rect);
            if look::tip(r, "Keep the picked one: one undo step", "Enter").clicked() {
                act = Act::Use;
            }
            let r = ui.button("Cancel");
            crate::drive::mark(ui.ctx(), "btn:wall.cancel", r.rect);
            if look::tip(r, "Drop the pick; it stays as it was", "Escape").clicked() {
                act = Act::Cancel;
            }
        }
        let r = ui.add_enabled(chosen.is_some(), egui::Button::new("More like this"));
        crate::drive::mark(ui.ctx(), "btn:wall.more", r.rect);
        let r = r.on_disabled_hover_text("Pick one first: a new set is made around the picked one");
        if let (true, Some(c)) = (look::tip(r, "A new set around the picked one", "M").clicked(), chosen) {
            act = Act::More(c);
        }
        if look::tip(ui.button("New batch"), "Other candidates around the same one", "").clicked() {
            act = Act::Shuffle;
        }
        let r = ui.add_enabled(wall.can_go_back(), egui::Button::new("Back"));
        if look::tip(r, "The set before the last \"More like this\"", "").clicked() {
            act = Act::Back;
        }
        // one piece, to the next line whole when it does not fit (a row inside a wrapping row cannot wrap)
        if ui.max_rect().right() - ui.cursor().left() < 300.0 {
            ui.end_row();
        } else {
            ui.add_space(GAP);
        }
        ui.horizontal(|ui| {
            ui.label(RichText::new("How different").small().color(look::TEXT2));
            ui.label(RichText::new("subtle").small().color(look::DISABLED));
            let r = ui.add(egui::Slider::new(&mut wall.spread, 0.05..=1.0).show_value(false));
            crate::drive::mark(ui.ctx(), "slider:spread", r.rect);
            if look::tip(r, "From subtle (left) to wild (right): how far the candidates may go from the first one", "").changed() {
                act = Act::Spread;
            }
            ui.label(RichText::new("wild").small().color(look::DISABLED));
        });
    });
    ui.add_space(GAP);
    // the keys, unless a field is typed into
    if !ctx.egui_wants_keyboard_input() {
        ctx.input_mut(|i| {
            if uses == Some(true) && i.consume_key(Modifiers::NONE, Key::Enter) {
                act = Act::Use;
            }
            if uses.is_some() && i.consume_key(Modifiers::NONE, Key::Escape) {
                act = Act::Cancel;
            }
            if let (true, Some(c)) = (i.consume_key(Modifiers::NONE, Key::M), chosen) {
                act = Act::More(c);
            }
        });
    }
    // the grid: the cells share the width evenly, no text under them
    let gap = GAP;
    let (cols, side) = fit(ui.available_width(), gap, cell.x, cell.x * 1.5);    let size = Vec2::new(side, (cell.y * side / cell.x).round());
    let n = wall.items.len();
    for row in 0..n.div_ceil(cols) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::splat(gap);
            for i in row * cols..(row * cols + cols).min(n) {
                let (r, resp) = ui.allocate_exact_size(size, Sense::click());
                crate::drive::mark(ui.ctx(), format!("wall:{i}"), r);
                let over = ui.rect_contains_pointer(r);
                let p = ui.painter().clone();
                p.rect_filled(r, 8.0, look::BG);
                draw(&p, r.shrink(8.0), i, &wall.items[i]);
                let edge = if chosen == Some(i) { Stroke::new(2.0, look::ACCENT) } else if over { Stroke::new(1.0, look::TEXT2) } else { Stroke::new(1.0, look::LINE) };
                p.rect_stroke(r, 8.0, edge, StrokeKind::Inside);
                if i == 0 {
                    badge(&p, r.left_top() + Vec2::splat(8.0), "Current");
                }
                if chosen == Some(i) {
                    let c = r.right_bottom() - Vec2::splat(14.0);
                    p.circle_filled(c, 9.0, look::ACCENT);
                    p.line(vec![c + Vec2::new(-4.0, 0.0), c + Vec2::new(-1.0, 3.0), c + Vec2::new(4.0, -3.0)], Stroke::new(2.0, Color32::WHITE));
                }
                let help = if i == 0 { "The current one, this set was made around it; a click picks it, a double click uses it" } else { "A click picks this one, a double click uses it" };
                let resp = look::tip(resp, help, "");
                if resp.double_clicked() {
                    act = if uses.is_some() { Act::Use } else { Act::Take(i) };
                } else if resp.clicked() {
                    act = Act::Take(i);
                }
                // on hover: "more like this" at the top right, play at the bottom left
                if over {
                    let more = Rect::from_center_size(r.right_top() + Vec2::new(-18.0, 18.0), Vec2::splat(24.0));
                    let mr = ui.interact(more, ui.scope_id().with(("wall.more", i)), Sense::click());
                    crate::drive::mark(ui.ctx(), format!("more:{i}"), more);
                    round(&p, more, mr.hovered());
                    // two offset cards: another like this
                    let c = more.center();
                    p.rect_stroke(Rect::from_center_size(c + Vec2::new(-2.0, -2.0), Vec2::splat(8.0)), 2.0, Stroke::new(1.5, look::TEXT2), StrokeKind::Middle);
                    p.rect_filled(Rect::from_center_size(c + Vec2::new(2.0, 2.0), Vec2::splat(8.0)), 2.0, Color32::WHITE);
                    if look::tip(mr, "More like this: a new set around this one", "").clicked() {
                        act = Act::More(i);
                    }
                    if play {
                        let pr = Rect::from_center_size(r.left_bottom() + Vec2::new(18.0, -18.0), Vec2::splat(24.0));
                        let resp = ui.interact(pr, ui.scope_id().with(("wall.play", i)), Sense::click());
                        crate::drive::mark(ui.ctx(), format!("play:{i}"), pr);
                        round(&p, pr, resp.hovered());
                        let c = pr.center();
                        p.add(egui::Shape::convex_polygon(vec![c + Vec2::new(-3.0, -5.0), c + Vec2::new(5.0, 0.0), c + Vec2::new(-3.0, 5.0)], Color32::WHITE, Stroke::NONE));
                        if look::tip(resp, "Hear this one", "").clicked() {
                            act = Act::Play(i);
                        }
                    }
                }
            }
        });
    }
    act
}

/// A round button's face on a picture.
fn round(p: &egui::Painter, r: Rect, hot: bool) {
    p.circle(r.center(), r.width() * 0.5, if hot { look::PRESSED } else { look::PANEL.gamma_multiply(0.92) }, Stroke::new(1.0, look::FIELD));
}

/// A small label pill on a picture.
fn badge(p: &egui::Painter, at: Pos2, text: &str) {
    let g = p.layout_no_wrap(text.into(), FontId::proportional(look::small()), look::TEXT);
    let r = Rect::from_min_size(at, g.size() + Vec2::new(12.0, 4.0));
    p.rect_filled(r, r.height() * 0.5, look::PANEL.gamma_multiply(0.92));
    p.galley(r.min + Vec2::new(6.0, 2.0), g, look::TEXT);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cells_fill_the_width_and_never_spill() {
        for (least, most) in [(140.0, 210.0), (60.0, 90.0), (170.0, 255.0), (100.0, 100.0)] {
            for w in (20..2400).step_by(7) {
                let (w, gap) = (w as f32, 12.0);
                let (cols, side) = fit(w, gap, least, most);
                let used = cols as f32 * side + (cols - 1) as f32 * gap;
                assert!(used <= w + 1e-3, "spills: {w} {least} {most} -> {cols} x {side}");
                assert!(side <= most, "too large: {w} -> {side}");
                assert!(w - used < side + gap + cols as f32, "more than a cell free: {w} {least} {most} -> {cols} x {side}");
                assert!(side >= least.min(w) - 1.0, "too small: {w} -> {side}");
            }
        }
    }
}
