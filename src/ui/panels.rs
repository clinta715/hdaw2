use crate::app::HdawApp;
use egui::Context;

/// Typed identifier for each panel in the registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelKind {
    Mixer,
    AudioPool,
    EffectEditor,
    PianoRoll,
    Preferences,
}

/// Ordered registry of panels. Iterates in insertion order during rendering.
/// Panels decide internally whether they are visible (e.g. checking state flags).
#[derive(Debug, Clone)]
pub struct PanelManager {
    panels: Vec<PanelKind>,
}

impl PanelManager {
    pub fn new() -> Self {
        let panels = vec![
            PanelKind::Mixer,
            PanelKind::AudioPool,
            PanelKind::EffectEditor,
            PanelKind::PianoRoll,
            PanelKind::Preferences,
        ];
        Self { panels }
    }
}

impl Default for PanelManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Renders every registered panel in order using the panel manager's
/// registry. Each panel's module checks its own visibility flags.
pub fn render_all(app: &mut HdawApp, ctx: &Context) {
    let panels = app.panel_manager.panels.clone();
    for &kind in &panels {
        match kind {
            PanelKind::Mixer => {
                if app.mixer_state.visible {
                    crate::ui::mixer_panel::render(ctx, app);
                }
            }
            PanelKind::AudioPool => {
                let mut state = std::mem::take(&mut app.audio_pool_state);
                crate::ui::audio_pool::render(ctx, &mut state, app);
                app.audio_pool_state = state;
            }
            PanelKind::EffectEditor => {
                crate::ui::effect_editor::render(ctx, app);
            }
            PanelKind::PianoRoll => {
                crate::ui::piano_roll::render(ctx, app);
            }
            PanelKind::Preferences => {
                crate::ui::preferences::render(ctx, app);
            }
        }
    }
}
