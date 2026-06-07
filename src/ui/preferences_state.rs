use std::path::PathBuf;

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub enum BufferSizePref {
    Small,
    #[default]
    Default,
    Large,
}

#[derive(Clone, Copy, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum GridDivision {
    #[default]
    Adaptive,
    Bar,
    Half,
    Quarter,
    Eighth,
    Sixteenth,
}

impl GridDivision {
    pub fn to_beats(self) -> f64 {
        match self {
            Self::Adaptive => 0.0,
            Self::Bar => 4.0,
            Self::Half => 2.0,
            Self::Quarter => 1.0,
            Self::Eighth => 0.5,
            Self::Sixteenth => 0.25,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Adaptive => "Adaptive",
            Self::Bar => "1 Bar",
            Self::Half => "1/2 Note",
            Self::Quarter => "1/4 Note",
            Self::Eighth => "1/8 Note",
            Self::Sixteenth => "1/16 Note",
        }
    }
}

#[derive(Clone, Copy)]
pub struct Theme {
    pub bg_fill: egui::Color32,
    pub grid_line: egui::Color32,
    pub grid_bar: egui::Color32,
    pub track_bg: egui::Color32,
    pub track_bg_alt: egui::Color32,
    pub text_normal: egui::Color32,
    pub text_dim: egui::Color32,
    pub clip_default: egui::Color32,
    pub selection: egui::Color32,
}

impl Theme {
    pub fn dark() -> Self {
        Self {
            bg_fill: egui::Color32::from_rgb(0x1e, 0x1e, 0x1e),
            grid_line: egui::Color32::from_rgba_premultiplied(80, 80, 90, 40),
            grid_bar: egui::Color32::from_rgba_premultiplied(100, 100, 120, 80),
            track_bg: egui::Color32::from_rgb(0x2c, 0x2c, 0x2c),
            track_bg_alt: egui::Color32::from_rgb(0x22, 0x22, 0x22),
            text_normal: egui::Color32::from_gray(220),
            text_dim: egui::Color32::from_gray(140),
            clip_default: egui::Color32::from_rgb(0x5c, 0x3a, 0x8a),
            selection: egui::Color32::from_rgb(0x64, 0xb5, 0xf6),
        }
    }
    pub fn light() -> Self {
        Self {
            bg_fill: egui::Color32::from_rgb(0xf0, 0xf0, 0xf0),
            grid_line: egui::Color32::from_rgba_premultiplied(160, 160, 170, 60),
            grid_bar: egui::Color32::from_rgba_premultiplied(120, 120, 140, 100),
            track_bg: egui::Color32::from_rgb(0xe0, 0xe0, 0xe0),
            track_bg_alt: egui::Color32::from_rgb(0xd4, 0xd4, 0xd4),
            text_normal: egui::Color32::from_gray(30),
            text_dim: egui::Color32::from_gray(120),
            clip_default: egui::Color32::from_rgb(0x9c, 0x7a, 0xca),
            selection: egui::Color32::from_rgb(0x19, 0x70, 0xd2),
        }
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct PreferencesState {
    pub show_dialog: bool,
    pub audio_device: String,
    pub sample_rate: u32,
    pub buffer_size: BufferSizePref,
    pub default_bpm: f64,
    pub default_time_sig_num: u8,
    pub default_time_sig_den: u8,
    pub default_zoom: f64,
    pub snap_default: bool,
    pub snap_to_markers: bool,
    pub grid_division: GridDivision,
    pub grid_opacity: f32,
    pub track_height: f32,
    pub header_width: f32,
    #[serde(alias = "show_mixer_on_start")]
    pub show_bottom_panel_on_start: bool,
    pub show_pool_on_start: bool,
    pub show_effect_editor_on_start: bool,
    pub effect_panel_width: f32,
    #[serde(default = "default_mixer_panel_height")]
    pub mixer_panel_height: f32,
    #[serde(default = "default_right_panel_width")]
    pub right_panel_width: f32,

    pub follow_playhead: bool,
    pub piano_roll_follow_playhead: bool,
    pub center_playhead_on_zoom: bool,

    pub piano_roll_row_height: f32,
    pub piano_roll_min_note: u8,
    pub piano_roll_max_note: u8,
    pub piano_roll_default_velocity: u8,

    #[serde(default)]
    pub last_import_dir: Option<PathBuf>,
    #[serde(default)]
    pub last_open_dir: Option<PathBuf>,
    #[serde(default)]
    pub last_save_dir: Option<PathBuf>,
    #[serde(default)]
    pub recent_files: Vec<PathBuf>,
    #[serde(default)]
    pub dark_mode: bool,
    #[serde(default)]
    pub sample_browser_bookmarks: Vec<PathBuf>,
}

const MIXER_PANEL_MIN: f32 = 160.0;
const MIXER_PANEL_MAX: f32 = 500.0;
const SIDE_PANEL_MIN: f32 = 140.0;
const SIDE_PANEL_MAX: f32 = 600.0;

fn default_mixer_panel_height() -> f32 { 220.0 }
fn default_right_panel_width() -> f32 { 220.0 }

impl PreferencesState {
    pub fn theme(&self) -> Theme {
        if self.dark_mode { Theme::dark() } else { Theme::light() }
    }
    pub fn push_recent_file(&mut self, path: PathBuf) {
        self.recent_files.retain(|p| p != &path);
        self.recent_files.insert(0, path);
        self.recent_files.truncate(10);
    }
    pub fn clamp_panel_sizes(&mut self) {
        self.mixer_panel_height = self.mixer_panel_height.clamp(MIXER_PANEL_MIN, MIXER_PANEL_MAX);
        self.right_panel_width = self.right_panel_width.clamp(SIDE_PANEL_MIN, SIDE_PANEL_MAX);
        self.effect_panel_width = self.effect_panel_width.clamp(SIDE_PANEL_MIN, SIDE_PANEL_MAX);
    }
}

impl Default for PreferencesState {
    fn default() -> Self {
        Self {
            show_dialog: false,
            audio_device: String::new(),
            sample_rate: 48000,
            buffer_size: BufferSizePref::Default,
            default_bpm: 120.0,
            default_time_sig_num: 4,
            default_time_sig_den: 4,
            default_zoom: 100.0,
            snap_default: true,
            snap_to_markers: true,
            grid_division: GridDivision::Adaptive,
            grid_opacity: 0.5,
            track_height: 80.0,
            header_width: 220.0,
            show_bottom_panel_on_start: true,
            show_pool_on_start: false,
            show_effect_editor_on_start: true,
            effect_panel_width: 280.0,
            mixer_panel_height: 220.0,
            right_panel_width: 220.0,

            follow_playhead: true,
            piano_roll_follow_playhead: true,
            center_playhead_on_zoom: true,

            piano_roll_row_height: 14.0,
            piano_roll_min_note: 24,
            piano_roll_max_note: 96,
            piano_roll_default_velocity: 100,

            last_import_dir: None,
            last_open_dir: None,
            last_save_dir: None,
            recent_files: Vec::new(),
            dark_mode: true,
            sample_browser_bookmarks: Vec::new(),
        }
    }
}
