use crate::app::{BottomPanelMode, HdawApp, TrackUiState};
use egui::{Color32, Context, Slider, RichText};
use std::sync::atomic::Ordering;
use uuid::Uuid;

const CHANNEL_WIDTH: f32 = 70.0;
const BOTTOM_MIN_HEIGHT: f32 = 160.0;
const METER_HEIGHT: f32 = 40.0;

pub struct BottomPanelState {
    pub master_volume: f32,
    pub visible: bool,
}

impl Default for BottomPanelState {
    fn default() -> Self {
        Self {
            master_volume: 1.0,
            visible: true,
        }
    }
}

pub fn render(ctx: &Context, app: &mut HdawApp) {
    if !app.bottom_panel_state.visible {
        return;
    }

    let mut panel = egui::TopBottomPanel::bottom("bottom_panel")
        .resizable(true)
        .min_height(BOTTOM_MIN_HEIGHT);

    panel = panel.default_height(app.preferences.mixer_panel_height);

    let panel_res = panel.show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    for (mode, label) in &[
                        (BottomPanelMode::Mixer, "Mixer"),
                        (BottomPanelMode::Sends, "Sends"),
                        (BottomPanelMode::FxChain, "FX Chain"),
                    ] {
                        let selected = app.bottom_panel_mode == *mode;
                        if ui.selectable_label(selected, *label).clicked() {
                            app.bottom_panel_mode = *mode;
                        }
                    }
                });
                ui.separator();

                match app.bottom_panel_mode {
                    BottomPanelMode::Mixer => render_mixer(ui, app),
                    BottomPanelMode::Sends => render_sends(ui, app),
                    BottomPanelMode::FxChain => render_fx_chain(ui, app),
                }
            });
        });

    let response_height = panel_res.response.rect.height();
    if response_height.is_finite() && (response_height - app.preferences.mixer_panel_height).abs() > 1.0 {
        app.preferences.mixer_panel_height = response_height;
    }
}

fn render_mixer(ui: &mut egui::Ui, app: &mut HdawApp) {
    egui::ScrollArea::horizontal()
        .id_salt("mixer_scroll")
        .auto_shrink([false, true])
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                let mv = app.master_volume();
                let new_mv = draw_master(ui, &mut app.bottom_panel_state, mv);
                if (new_mv - mv).abs() > 0.001 {
                    app.set_master_volume(new_mv);
                }
                ui.add_space(4.0);
                let all_tracks = app.track_ui.clone();
                for i in 0..all_tracks.len() {
                    draw_channel(ui, i, app, &all_tracks);
                    ui.add_space(4.0);
                }
            });
        });
}

fn draw_master(ui: &mut egui::Ui, state: &mut BottomPanelState, initial_vol: f32) -> f32 {
    state.master_volume = initial_vol;
    ui.vertical(|ui| {
        ui.set_width(CHANNEL_WIDTH);

        ui.label(RichText::new("Master").strong().size(11.0));

        let mw = CHANNEL_WIDTH - 12.0;
        let (meter_rect, _) = ui.allocate_exact_size(egui::vec2(mw, METER_HEIGHT), egui::Sense::hover());
        draw_vu_meter(ui, meter_rect, state.master_volume, false);

        ui.add_sized(
            egui::vec2(ui.available_width(), 120.0),
            Slider::new(&mut state.master_volume, 0.0..=1.0)
                .vertical()
                .show_value(false),
        );

        ui.label(format!("{:.1} dB", 20.0 * state.master_volume.max(0.0001).log10()));
    });
    state.master_volume
}

fn draw_channel(ui: &mut egui::Ui, index: usize, app: &mut HdawApp, all_tracks: &[TrackUiState]) {
    let tui = &all_tracks[index];
    let muted = tui.mute.load(Ordering::Acquire);
    let solo = tui.solo.load(Ordering::Acquire);
    let peak_l = f32::from_bits(tui.peak_left.load(Ordering::Acquire));
    let peak_r = f32::from_bits(tui.peak_right.load(Ordering::Acquire));
    let peak = peak_l.max(peak_r);
    let is_group = tui.is_group;
    let is_return = tui.is_return;
    let parent_group = tui.parent_group;
    let send_levels_count = tui.send_levels.len();
    let mut vol = f32::from_bits(tui.volume.load(Ordering::Acquire));
    let color = Color32::from_rgb(tui.color[0], tui.color[1], tui.color[2]);

    ui.vertical(|ui| {
        ui.set_width(CHANNEL_WIDTH);

        ui.horizontal(|ui| {
            let pad = (CHANNEL_WIDTH * 0.5 - 4.0).max(0.0);
            ui.add_space(pad);
            ui.label(RichText::new(&tui.name).size(10.0).color(color));
        });

        let mw = CHANNEL_WIDTH - 12.0;
        let (meter_rect, _) = ui.allocate_exact_size(egui::vec2(mw, METER_HEIGHT), egui::Sense::hover());
        draw_vu_meter(ui, meter_rect, peak, muted);

        let response = ui.add_sized(
            egui::vec2(ui.available_width(), 120.0),
            Slider::new(&mut vol, 0.0..=1.0)
                .vertical()
                .show_value(false),
        );
        if response.changed() {
            app.track_ui[index].volume.store(vol.to_bits(), Ordering::Release);
            if let Some(track) = app.project.tracks.get_mut(index) {
                track.volume = vol;
            }
        }

        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(2.0, 0.0);

            let mf = if muted { Color32::from_rgb(0xcc, 0x33, 0x33) } else { Color32::from_gray(50) };
            let mc = if muted { Color32::WHITE } else { Color32::from_gray(160) };
            if ui.add(egui::Button::new(RichText::new("M").size(10.0).color(mc)).small().fill(mf)).clicked() {
                app.toggle_track_mute(index);
            }

            let sf = if solo { Color32::from_rgb(0xcc, 0xcc, 0x33) } else { Color32::from_gray(50) };
            let sc = if solo { Color32::BLACK } else { Color32::from_gray(160) };
            if ui.add(egui::Button::new(RichText::new("S").size(10.0).color(sc)).small().fill(sf)).clicked() {
                app.toggle_track_solo(index);
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(format!("{:.1}", vol)).size(9.0).color(Color32::from_gray(180)));
            });
        });

        if !is_group && !is_return {
            ui.add_space(2.0);
            let groups: Vec<(Uuid, String)> = all_tracks.iter()
                .filter(|t| t.is_group)
                .map(|t| (t.id, t.name.clone()))
                .collect();
            let current_label = if let Some(pid) = parent_group {
                groups.iter().find(|(id, _)| *id == pid).map(|(_, n)| n.clone())
                    .unwrap_or_else(|| "Master".to_string())
            } else {
                "Master".to_string()
            };
            ui.menu_button(format!("RTE: {}", current_label), |ui| {
                if ui.button("Master").clicked() {
                    app.set_track_parent(index, None);
                    ui.close_menu();
                }
                ui.separator();
                for (gid, gname) in &groups {
                    if ui.button(gname).clicked() {
                        app.set_track_parent(index, Some(*gid));
                        ui.close_menu();
                    }
                }
            });
        }

        if send_levels_count > 0 {
            ui.add_space(2.0);
            ui.label(RichText::new("Sends").small().color(Color32::from_gray(160)));
            let project_sends = app.project.tracks.get(index).map(|t| t.sends.clone()).unwrap_or_default();
            for si in 0..send_levels_count.min(4) {
                let mut level = f32::from_bits(app.track_ui[index].send_levels[si].load(Ordering::Acquire));
                let return_name = project_sends.get(si)
                    .and_then(|s| all_tracks.iter().find(|t| t.id == s.target_id))
                    .map(|t| if t.name.len() > 6 { format!("{}…", &t.name[..6]) } else { t.name.clone() })
                    .unwrap_or_else(|| "?".to_string());
                let resp = ui.add(Slider::new(&mut level, 0.0..=1.0).text(return_name));
                if resp.changed() {
                    app.set_send_level(index, si, level);
                }
            }
        }
    });
}

fn render_sends(ui: &mut egui::Ui, app: &mut HdawApp) {
    ui.label(RichText::new("Send Routing").strong().size(11.0));
    ui.add_space(4.0);

    let returns: Vec<(Uuid, String)> = app.track_ui.iter()
        .filter(|t| t.is_return)
        .map(|t| (t.id, t.name.clone()))
        .collect();

    if returns.is_empty() {
        ui.label("No return tracks. Add a return track to create sends.");
        return;
    }

    egui::ScrollArea::horizontal().show(ui, |ui| {
        egui::Grid::new("sends_grid")
            .striped(true)
            .min_col_width(60.0)
            .show(ui, |grid| {
                grid.colored_label(Color32::WHITE, "Track");
                for (_, rn) in &returns {
                    grid.colored_label(Color32::WHITE, rn);
                }
                grid.end_row();

                for ti in 0..app.track_ui.len() {
                    let is_return = app.track_ui[ti].is_return;
                    if is_return { continue; }
                    grid.label(&app.track_ui[ti].name);
                    for si in 0..returns.len() {
                        let mut level = if si < app.track_ui[ti].send_levels.len() {
                            f32::from_bits(app.track_ui[ti].send_levels[si].load(Ordering::Acquire))
                        } else {
                            0.0
                        };
                        let resp = grid.add(Slider::new(&mut level, 0.0..=1.0).show_value(false));
                        if resp.changed() {
                            if si < app.track_ui[ti].send_levels.len() {
                                app.track_ui[ti].send_levels[si].store(level.to_bits(), Ordering::Release);
                            }
                            app.set_send_level(ti, si, level);
                        }
                    }
                    grid.end_row();
                }
            });
    });
}

fn render_fx_chain(ui: &mut egui::Ui, app: &mut HdawApp) {
    let track_idx = match app.selected_track {
        Some(i) => i,
        None => {
            ui.label("No track selected.");
            return;
        }
    };

    let track_name = app.track_ui.get(track_idx).map(|t| &t.name).cloned().unwrap_or_default();
    ui.label(RichText::new(format!("FX Chain: {}", track_name)).strong().size(11.0));
    ui.add_space(4.0);

    if let Ok(tracks) = app.engine.tracks.lock() {
        if let Some(track) = tracks.get(track_idx) {
            if track.fx_chain.is_empty() {
                ui.label("No effects.");
                return;
            }
            egui::ScrollArea::vertical().show(ui, |ui| {
                for (fi, fx) in track.fx_chain.iter().enumerate() {
                    ui.horizontal(|ui| {
                        let bypass = fx.bypass.load(Ordering::Acquire);
                        let mut b = bypass;
                        if ui.checkbox(&mut b, "").changed() {
                            fx.bypass.store(b, Ordering::Release);
                        }
                        ui.label(format!("{}: {}", fi + 1, fx.name));
                    });
                    for info in fx.parameter_info().iter() {
                        let val = fx.parameter_value(info.id);
                        let mut val_local = val;
                        let resp = ui.add(Slider::new(&mut val_local, 0.0..=1.0).text(&info.name));
                        if resp.changed() {
                            fx.try_set_parameter(info.id, val_local);
                        }
                    }
                    ui.separator();
                }
            });
        }
    }
}

fn draw_vu_meter(ui: &mut egui::Ui, rect: egui::Rect, level: f32, muted: bool) {
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, Color32::from_rgb(0x12, 0x12, 0x12));

    if muted {
        painter.rect_filled(rect, 0.0, Color32::from_rgb(0x22, 0x22, 0x22));
        painter.rect_stroke(rect, 0.0, egui::Stroke::new(1.0, Color32::from_gray(50)));
        return;
    }

    let clamped = level.min(1.2);
    if clamped > 0.0 {
        let fill_height = (rect.height() * (clamped / 1.0)).min(rect.height());
        let fill_rect = egui::Rect::from_min_max(
            egui::pos2(rect.left(), rect.bottom() - fill_height),
            rect.right_bottom(),
        );

        let color = if clamped > 1.0 {
            Color32::from_rgb(0xff, 0x00, 0x00)
        } else if clamped > 0.8 {
            Color32::from_rgb(0xcc, 0xaa, 0x33)
        } else {
            Color32::from_rgb(0x4c, 0xaf, 0x50)
        };
        painter.rect_filled(fill_rect, 0.0, color);
    }

    for i in 0..=10 {
        let y = rect.bottom() - (rect.height() * i as f32 / 10.0);
        let tick_w = if i % 5 == 0 { 4.0 } else { 2.0 };
        painter.line_segment(
            [egui::pos2(rect.right(), y), egui::pos2(rect.right() - tick_w, y)],
            egui::Stroke::new(1.0, Color32::from_gray(80)),
        );
    }

    painter.rect_stroke(rect, 0.0, egui::Stroke::new(1.0, Color32::from_gray(60)));
}
