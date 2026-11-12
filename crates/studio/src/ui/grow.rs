//! The palette grows where you paint (docs/design/studio.md, second round step 8): under the swatches of every
//! page that paints, a search box and the count, "+ New colour" (a small picker and a name), "Add shades" and
//! "Remove". What they change goes to the Palette page as its commands (`add:<name>=#rrggbb`, `shades:<name>`,
//! `remove:<name>`, `undo:<step>`), which the frame hands it after the page is drawn: the palette's checks, its file
//! and its undo stay that page's.
use super::look::{self, c32, GAP};
use eframe::egui::{self, Color32, RichText, Sense, StrokeKind, Ui, Vec2};
use engine::palette::{hex, to_hex, Palette};

/// The most colours one model can hold: its voxel rows have a letter per colour (`engine::models::LETTERS`).
/// The palette itself has no limit.
pub const MODEL_MAX: usize = engine::models::LETTERS.len();

#[derive(Clone, Copy, Default, PartialEq)]
enum Open {
    #[default]
    None,
    Add,
    Shades,
}

#[derive(Clone, Default)]
struct State {
    open: Open,
    hsv: [f32; 3],
    hex: String,
    name: String,
    /// The name last suggested: while the name is still it, it follows the colour.
    suggested: String,
    search: String,
    /// The last change made here, for its Undo button: its words, and its step's name in the palette's history.
    undo: Option<(String, &'static str)>,
}

fn queue() -> egui::Id {
    egui::Id::unique("palette commands")
}

fn send(ctx: &egui::Context, cmd: String) {
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Vec<String>>(queue()).push(cmd));
}

/// The Palette page's commands sent this frame, for the frame to run.
pub fn take(ctx: &egui::Context) -> Vec<String> {
    ctx.data_mut(|d| d.remove_temp::<Vec<String>>(queue())).unwrap_or_default()
}

/// The state of the swatches named `key` (a section's ui is not the same from frame to frame, so not its id).
fn state(ui: &Ui, key: &str) -> (egui::Id, State) {
    let id = egui::Id::unique(("palette grows", key));
    (id, ui.ctx().data_mut(|d| d.get_temp::<State>(id)).unwrap_or_default())
}

/// The search box and the count over the swatches; the text to filter names by (lower case).
pub fn search(ui: &mut Ui, key: &str, pal: &Palette) -> String {
    let (id, mut st) = state(ui, key);
    ui.horizontal(|ui| {
        let r = ui.add(egui::TextEdit::singleline(&mut st.search).id(egui::Id::unique(("palette search", key))).hint_text("find a colour").desired_width((ui.available_width() - 110.0).clamp(80.0, 220.0)));
        crate::drive::mark(ui.ctx(), "palette.search", r.rect);
        look::tip(r, "Type part of a colour's or a ramp's name: only those swatches show", "");
        if !st.search.is_empty() && look::tip(ui.small_button("clear"), "Show every colour again", "").clicked() {
            st.search.clear();
        }
        let n = pal.colours.len();
        look::tip(ui.label(RichText::new(format!("{n} colours")).small().weak()), &format!("The palette has no limit; one model holds at most {MODEL_MAX} of its colours"), "");
    });
    let q = st.search.trim().to_lowercase();
    ui.ctx().data_mut(|d| d.insert_temp(id, st));
    q
}

/// Whether colour `i` shows under search `q`: its name or its ramp's has it.
pub fn shown(pal: &Palette, i: usize, q: &str) -> bool {
    let c = &pal.colours[i];
    q.is_empty() || c.name.contains(q) || c.ramp.as_deref().is_some_and(|r| r.contains(q))
}

/// The row under the swatches: "+ New colour", "Add shades" and "Remove" for the first colour, and the open
/// picker or shades preview. Some(the new colour) when Add took one: it is the first colour now.
pub fn grow(ui: &mut Ui, key: &str, pal: &Palette, first: &mut [u8; 3]) -> Option<[u8; 3]> {
    let (id, mut st) = state(ui, key);
    let chosen = pal.find(*first).map(|i| pal.colours[i].clone());
    let mut added = None;
    ui.add_space(GAP * 0.5);
    ui.horizontal_wrapped(|ui| {
        let r = ui.selectable_label(st.open == Open::Add, "+ New colour");
        crate::drive::mark(ui.ctx(), "btn:palette.new", r.rect);
        if look::tip(r, "Add a colour to the palette: pick it, name it, Add; it becomes your first colour", "").clicked() {
            st.open = if st.open == Open::Add { Open::None } else { Open::Add };
            let h = crate::ramp::hsv(*first);
            st.hsv = h;
            st.hex = to_hex(*first);
            st.suggested = crate::ramp::suggest(pal, *first);
            st.name = st.suggested.clone();
        }
        let r = ui.add_enabled(chosen.is_some(), egui::Button::selectable(st.open == Open::Shades, "Add shades"));
        crate::drive::mark(ui.ctx(), "btn:palette.shades", r.rect);
        if look::tip(r, "Four shades of the first colour, two darker and two lighter, shown before you Use them; they join its ramp", "").clicked() {
            st.open = if st.open == Open::Shades { Open::None } else { Open::Shades };
        }
        let r = ui.add_enabled(chosen.is_some(), egui::Button::new("Remove"));
        crate::drive::mark(ui.ctx(), "btn:palette.remove", r.rect);
        if look::tip(r, "Take the first colour out of the palette; refused, with the count, while a texture or a model uses it", "").clicked() {
            if let Some(c) = &chosen {
                send(ui.ctx(), format!("remove:{}", c.name));
                st.undo = Some((format!("Remove {}", c.name), "Remove"));
            }
        }
        if let Some((u, step)) = st.undo.clone() {
            let r = ui.button(format!("Undo: {u}"));
            crate::drive::mark(ui.ctx(), "btn:palette.undo", r.rect);
            if look::tip(r, "Undo the palette's last change (the Palette page's history has every step)", "").clicked() {
                send(ui.ctx(), format!("undo:{step}"));
                st.undo = None;
            }
        }
    });
    match st.open {
        Open::Add => added = picker(ui, key, pal, &mut st),
        Open::Shades => shades(ui, pal, chosen.as_ref().map(|c| c.name.as_str()), &mut st),
        Open::None => {}
    }
    if let Some(c) = added {
        *first = c;
    }
    ui.ctx().data_mut(|d| d.insert_temp(id, st));
    added
}

/// The small picker: hue, saturation and value, the hex, and a name with a suggestion. The text boxes have ids
/// of their own: the section's layout ids change between frames, and a box that changes id loses the focus.
fn picker(ui: &mut Ui, key: &str, pal: &Palette, st: &mut State) -> Option<[u8; 3]> {
    let mut out = None;
    egui::Frame::group(ui.style()).show(ui, |ui| {
        let rgb = crate::ramp::rgb(st.hsv);
        let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width().min(260.0), 32.0), Sense::hover());
        ui.painter().rect_filled(r, 4.0, c32(rgb));
        ui.painter().rect_stroke(r, 4.0, egui::Stroke::new(1.0, Color32::from_gray(90)), StrokeKind::Inside);
        let mut moved = false;
        for (k, label) in ["hue", "saturation", "value"].into_iter().enumerate() {
            let r = look::row(ui, label, |ui| {
                ui.spacing_mut().slider_width = (ui.available_width() - 8.0).clamp(40.0, 200.0);
                ui.add(egui::Slider::new(&mut st.hsv[k], 0.0..=1.0).show_value(false))
            });
            crate::drive::mark(ui.ctx(), format!("palette.{label}"), r.rect);
            moved |= look::tip(r, &format!("The new colour's {label}"), "").changed();
        }
        if moved {
            st.hex = to_hex(crate::ramp::rgb(st.hsv));
        }
        let r = look::row(ui, "hex", |ui| ui.add(egui::TextEdit::singleline(&mut st.hex).id(egui::Id::unique(("palette hex", key))).desired_width(90.0)));
        crate::drive::mark(ui.ctx(), "palette.hex", r.rect);
        if look::tip(r, "The colour as #rrggbb: type or paste one", "").changed() {
            if let Some(c) = hex(st.hex.trim()) {
                st.hsv = crate::ramp::hsv(c);
                moved = true;
            }
        }
        let rgb = crate::ramp::rgb(st.hsv);
        // the name follows the colour until it is typed in
        if moved && st.name == st.suggested {
            st.suggested = crate::ramp::suggest(pal, rgb);
            st.name = st.suggested.clone();
        }
        let r = look::row(ui, "name", |ui| ui.add(egui::TextEdit::singleline(&mut st.name).id(egui::Id::unique(("palette name", key))).desired_width(f32::INFINITY)));
        crate::drive::mark(ui.ctx(), "palette.name", r.rect);
        look::tip(r, "The colour's name in the palette: lower case, unique; a suggestion from its hue until you type one", "");
        let name = st.name.trim().to_string();
        let ok = pal.check(&name, rgb, None);
        if let Err(e) = &ok {
            ui.label(RichText::new(format!("{e}: move a slider or change the name")).small().color(look::BAD));
        }
        ui.horizontal(|ui| {
            let r = ui.add_enabled(ok.is_ok(), egui::Button::new("Add"));
            crate::drive::mark(ui.ctx(), "btn:palette.add", r.rect);
            if look::tip(r, "Write the colour into data/palette.toml and make it your first colour (Undo takes it back)", "").clicked() {
                send(ui.ctx(), format!("add:{name}={}", to_hex(rgb)));
                st.undo = Some((format!("Add {name}"), "Add"));
                st.open = Open::None;
                out = Some(rgb);
            }
            if look::tip(ui.button("Cancel"), "Close the picker; nothing is added", "").clicked() {
                st.open = Open::None;
            }
        });
    });
    out
}

/// The shades preview: the ramp as it would be, the new shades marked, and Use.
fn shades(ui: &mut Ui, pal: &Palette, name: Option<&str>, st: &mut State) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        let Some(name) = name else {
            look::weak(ui, "Choose a palette colour first: its shades are made from it");
            return;
        };
        let i = pal.colours.iter().position(|c| c.name == name).unwrap();
        match crate::ramp::shades(pal, i) {
            Ok((next, added)) => {
                let ramp = next.colours.iter().find(|c| c.name == added[0]).and_then(|c| c.ramp.clone()).unwrap_or_default();
                ui.label(RichText::new(format!("The ramp {ramp} with four new shades of {name} (marked):")).small());
                let run = next.ramp(&ramp);
                let size = ((ui.available_width() - 4.0) / run.len() as f32).clamp(16.0, 36.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::splat(2.0);
                    for &k in &run {
                        let c = &next.colours[k];
                        let (r, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
                        ui.painter().rect_filled(r, 3.0, c32(c.rgb));
                        if added.contains(&c.name) {
                            ui.painter().rect_stroke(r, 3.0, egui::Stroke::new(2.0, look::ACCENT), StrokeKind::Outside);
                        }
                        look::tip(resp, &format!("{} {}{}", c.name, to_hex(c.rgb), if added.contains(&c.name) { " (new)" } else { "" }), "");
                    }
                });
                look::weak(ui, format!("new: {}", added.join(", ")));
                ui.horizontal(|ui| {
                    let r = ui.button("Use");
                    crate::drive::mark(ui.ctx(), "btn:palette.use", r.rect);
                    if look::tip(r, "Add these shades to the palette: Shade and the patterns' auto colours walk them at once (Undo takes them back)", "").clicked() {
                        send(ui.ctx(), format!("shades:{name}"));
                        st.undo = Some((format!("Add shades of {name}"), "Add shades"));
                        st.open = Open::None;
                    }
                    if look::tip(ui.button("Cancel"), "Close the preview; nothing is added", "").clicked() {
                        st.open = Open::None;
                    }
                });
            }
            Err(e) => {
                ui.label(RichText::new(e).small().color(look::BAD));
            }
        }
    });
}
