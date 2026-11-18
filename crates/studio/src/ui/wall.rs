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

/// The keys a wall took last frame, by [`key_act`]: those are then the wall's, not the page's.
pub fn claims(ctx: &egui::Context, key: Key) -> bool {
    let last = ctx.data(|d| d.get_temp::<(u64, Option<bool>, Option<usize>)>(egui::Id::unique("wall.keys")));
    last.is_some_and(|(f, uses, chosen)| f + 1 >= ctx.cumulative_frame_nr() && key_act(key, uses, chosen).is_some())
}

/// Set by the frame before the page runs: whether a page may take keys now (no overlay, ask or closing).
pub fn allow_keys(ctx: &egui::Context, on: bool) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique("wall.allowed"), on));
}

/// `show_with` where `uses` is `Some(picked)` on a page whose pick waits for Use: the bar then leads with
/// Use (on while something is picked) and Cancel.
pub fn show_full<T: Clone>(ui: &mut Ui, wall: &mut Wall<T>, chosen: Option<usize>, cell: Vec2, play: bool, uses: Option<bool>, draw: impl Fn(&egui::Painter, Rect, usize, &T)) -> Act {
    let mut act = Act::None;
    let ctx = ui.ctx().clone();
    // the one bar
    let bar = ui.horizontal_wrapped(|ui| {
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
    // the keys: only while a page may take keys (no overlay or ask over it), no field is typed into, and the
    // bar is in sight (the work's scroll area runs a wall scrolled away too)
    let allowed = ctx.data(|d| d.get_temp::<bool>(egui::Id::unique("wall.allowed"))).unwrap_or(true);
    if allowed && !ctx.egui_wants_keyboard_input() && ui.clip_rect().intersects(bar.response.rect) {
        let frame = ctx.cumulative_frame_nr();
        ctx.data_mut(|d| d.insert_temp(egui::Id::unique("wall.keys"), (frame, uses, chosen)));
        ctx.input_mut(|i| {
            for key in [Key::Enter, Key::Escape, Key::M] {
                if let Some(a) = key_act(key, uses, chosen).filter(|_| i.consume_key(Modifiers::NONE, key)) {
                    act = a;
                }
            }
        });
    }
    // the grid: the cells share the width evenly, no text under them
    let gap = GAP;
    let (cols, side) = fit(ui.available_width(), gap, cell.x, cell.x * 1.5);
    let size = Vec2::new(side, (cell.y * side / cell.x).round());
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
                if resp.clicked() || resp.double_clicked() {
                    act = click_act(i, resp.double_clicked(), uses);
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

/// What a key does on the wall; `None` leaves the key to the page.
fn key_act(key: Key, uses: Option<bool>, chosen: Option<usize>) -> Option<Act> {
    match key {
        Key::Enter => (uses == Some(true)).then_some(Act::Use),
        Key::Escape => uses.is_some().then_some(Act::Cancel),
        Key::M => chosen.map(Act::More),
        _ => None,
    }
}

/// What a click on item `i` does (a double click comes as a second click).
fn click_act(i: usize, double: bool, uses: Option<bool>) -> Act {
    // (a double click on Current only picks it: on Maps that is "nothing picked", and Use would close the wall)
    if double && uses.is_some() && i > 0 { Act::Use } else { Act::Take(i) }
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
    fn m_is_the_walls_only_when_something_is_picked_and_a_double_click_on_current_keeps_the_wall() {
        assert_eq!(key_act(Key::M, None, Some(2)), Some(Act::More(2)));
        assert_eq!(key_act(Key::M, Some(false), None), None, "nothing picked: M is the page's (Mirror, vary, mutate)");
        assert_eq!(key_act(Key::Enter, Some(false), None), None);
        assert_eq!(key_act(Key::Escape, None, Some(1)), None, "no pending pick: Escape is the page's");
        assert_eq!(click_act(3, true, Some(true)), Act::Use);
        assert_eq!(click_act(0, true, Some(false)), Act::Take(0), "a double click on Current only picks it, the wall stays");
        assert_eq!(click_act(3, true, None), Act::Take(3));
    }

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
