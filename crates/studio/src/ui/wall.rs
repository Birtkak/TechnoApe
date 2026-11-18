//! The variations wall as one widget (docs/design/studio-tools.md "The variations wall"): a page hands it a
//! [`Wall`] and a way to draw one item; it shows the spread, "back", "other candidates" and the grid, and says
//! what was clicked. The page then takes the item, or refills the wall with its own generator.
use super::look::{self, GAP};
use crate::wall::Wall;
use eframe::egui::{self, Color32, Rect, RichText, Sense, StrokeKind, Ui, Vec2};

#[derive(PartialEq, Debug)]
pub enum Act {
    None,
    /// Take this item into the editor.
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
}

/// `chosen`: the item that is in the editor now (outlined). `cell`: the size of one preview.
pub fn show<T: Clone>(ui: &mut Ui, wall: &mut Wall<T>, chosen: Option<usize>, cell: Vec2, draw: impl Fn(&egui::Painter, Rect, &T)) -> Act {
    show_with(ui, wall, chosen, cell, false, |p, r, _, t| draw(p, r, t))
}

/// `show` with a play button under each item when `play`, and the item's index given to `draw`.
pub fn show_with<T: Clone>(ui: &mut Ui, wall: &mut Wall<T>, chosen: Option<usize>, cell: Vec2, play: bool, draw: impl Fn(&egui::Painter, Rect, usize, &T)) -> Act {
    let mut act = Act::None;
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Variations").strong());
        ui.add_space(GAP);
        ui.label(RichText::new("How different").small());
        let r = ui.add(egui::Slider::new(&mut wall.spread, 0.05..=1.0).show_value(false));
        crate::drive::mark(ui.ctx(), "slider:spread", r.rect);
        if look::tip(r, "From slight (left) to strong (right): how far the candidates may go from the first one", "").changed() {
            act = Act::Spread;
        }
        let r = ui.add_enabled(wall.can_go_back(), egui::Button::new("Back"));
        if look::tip(r, "The wall before the last \"More like this\"", "").clicked() {
            act = Act::Back;
        }
        if look::tip(ui.button("New batch"), "Other candidates around the same first one", "").clicked() {
            act = Act::Shuffle;
        }
    });
    ui.add_space(GAP * 0.5);
    let gap = GAP;
    let cols = ((ui.available_width() + gap) / (cell.x + gap)).floor().max(1.0) as usize;
    let n = wall.items.len();
    for row in 0..n.div_ceil(cols) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::splat(gap);
            for i in row * cols..(row * cols + cols).min(n) {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::splat(4.0);
                    let (r, resp) = ui.allocate_exact_size(cell, Sense::click());
                    crate::drive::mark(ui.ctx(), format!("wall:{i}"), r);
                    let p = ui.painter();
                    p.rect_filled(r, 6.0, ui.visuals().extreme_bg_color);
                    draw(p, r.shrink(6.0), i, &wall.items[i]);
                    if chosen == Some(i) {
                        p.rect_stroke(r, 6.0, egui::Stroke::new(2.0, look::ACCENT), StrokeKind::Inside);
                    } else if resp.hovered() {
                        p.rect_stroke(r, 6.0, egui::Stroke::new(1.0, Color32::from_gray(120)), StrokeKind::Inside);
                    }
                    let help = if i == 0 { "As it is now: the one this wall was made around; a click takes it back" } else { "A click picks this one" };
                    if look::tip(resp, help, "").clicked() {
                        act = Act::Take(i);
                    }
                    ui.horizontal(|ui| {
                        ui.set_width(cell.x);
                        if play {
                            let r = ui.add(egui::Button::new(RichText::new("play").small()).frame(false));
                            if look::tip(r, "Hear this one", "").clicked() {
                                act = Act::Play(i);
                            }
                        }
                        if i == 0 && !play {
                            look::tip(look::weak(ui, "as now"), "The one this wall was made around, unchanged", "");
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let r = ui.add(egui::Button::new(RichText::new(if play && cell.x < 120.0 || i == 0 && !play && cell.x < 170.0 { "More" } else { "More like this" }).small()).frame(false));
                            if look::tip(r, "A new wall made around this one", "").clicked() {
                                act = Act::More(i);
                            }
                        });
                    });
                });
            }
        });
    }
    act
}
