use egui::{Button, Context};

pub struct ToolbarState;

impl Default for ToolbarState {
    fn default() -> Self {
        Self
    }
}

pub struct ToolbarAction {
    pub play_clicked: bool,
    pub pause_clicked: bool,
    pub stop_clicked: bool,
    pub import_clicked: bool,
    pub fx_clicked: bool,
    pub new_clicked: bool,
    pub save_clicked: bool,
    pub save_as_clicked: bool,
    pub open_clicked: bool,
    pub snap_clicked: bool,
    pub undo_clicked: bool,
    pub redo_clicked: bool,
    pub loop_clicked: bool,
    pub add_track_clicked: bool,
    pub delete_track_clicked: bool,
    pub mixer_clicked: bool,
    pub pool_clicked: bool,
    pub preferences_clicked: bool,
}

pub fn render(
    ctx: &Context,
    is_playing: bool,
    position_secs: f64,
    bpm: f64,
    time_sig_num: u8,
    time_sig_den: u8,
    snap_enabled: bool,
    can_undo: bool,
    can_redo: bool,
    loop_enabled: bool,
    mixer_visible: bool,
    has_selected_track: bool,
    pool_visible: bool,
) -> ToolbarAction {
    let mut action = ToolbarAction {
        play_clicked: false,
        pause_clicked: false,
        stop_clicked: false,
        import_clicked: false,
        fx_clicked: false,
        new_clicked: false,
        save_clicked: false,
        save_as_clicked: false,
        open_clicked: false,
        snap_clicked: false,
        undo_clicked: false,
        redo_clicked: false,
        loop_clicked: false,
        add_track_clicked: false,
        delete_track_clicked: false,
        mixer_clicked: false,
        pool_clicked: false,
        preferences_clicked: false,
    };

    egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.label("HDAW");
            ui.separator();

            ui.menu_button("Edit", |ui| {
                if ui.add_enabled(can_undo, egui::Button::new("Undo")).clicked() {
                    action.undo_clicked = true;
                    ui.close_menu();
                }
                if ui.add_enabled(can_redo, egui::Button::new("Redo")).clicked() {
                    action.redo_clicked = true;
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Preferences...").clicked() {
                    action.preferences_clicked = true;
                    ui.close_menu();
                }
            });

            ui.menu_button("View", |ui| {
                let mut mixer_vis = mixer_visible;
                if ui.checkbox(&mut mixer_vis, "Mixer").clicked() {
                    action.mixer_clicked = true;
                    ui.close_menu();
                }
                let mut pool_state = pool_visible;
                if ui.checkbox(&mut pool_state, "Audio Pool").clicked() {
                    action.pool_clicked = true;
                    ui.close_menu();
                }
            });

            ui.menu_button("File", |ui| {
                if ui.button("New Project").clicked() {
                    action.new_clicked = true;
                    ui.close_menu();
                }
                if ui.button("Open...").clicked() {
                    action.open_clicked = true;
                    ui.close_menu();
                }
                if ui.button("Save").clicked() {
                    action.save_clicked = true;
                    ui.close_menu();
                }
                if ui.button("Save As...").clicked() {
                    action.save_as_clicked = true;
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Import Audio...").clicked() {
                    action.import_clicked = true;
                    ui.close_menu();
                }
            });

            if ui
                .add(Button::new(if is_playing { "\u{23F8}" } else { "\u{25B6}" }))
                .clicked()
            {
                if is_playing {
                    action.pause_clicked = true;
                } else {
                    action.play_clicked = true;
                }
            }
            if ui.button("\u{25A0}").clicked() {
                action.stop_clicked = true;
            }

            ui.separator();

            let mins = (position_secs / 60.0) as u32;
            let secs = (position_secs % 60.0) as u32;
            let millis = ((position_secs % 1.0) * 1000.0) as u32;
            ui.monospace(format!("{:02}:{:02}.{:03}", mins, secs, millis));

            ui.separator();

            ui.label(format!("BPM {:.1}", bpm));
            ui.label(format!("{} / {}", time_sig_num, time_sig_den));

            ui.separator();

            let snap_label = if snap_enabled { "Snap" } else { "Snap" };
            if ui
                .add(egui::Button::new(snap_label).selected(snap_enabled))
                .clicked()
            {
                action.snap_clicked = true;
            }

            ui.separator();

            if ui.add(egui::Button::new("+").small()).clicked() {
                action.add_track_clicked = true;
            }
            if ui.add_enabled(has_selected_track, egui::Button::new("-").small()).clicked() {
                action.delete_track_clicked = true;
            }

            ui.separator();

            if ui
                .add(egui::Button::new("\u{21BA}")  // loop arrow symbol
                    .selected(loop_enabled))
                .clicked()
            {
                action.loop_clicked = true;
            }

            if ui.button("FX").clicked() {
                action.fx_clicked = true;
            }
        });
        ui.add_space(2.0);
    });

    action
}
