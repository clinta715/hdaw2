# Tiled Layout Architecture

Date: 2026-06-05
Status: Approved for implementation

## Overview

Replace floating-window-based editors (piano roll) with an Ableton Live-style tiled layout. The main window is divided into three panels: a central **Main Tile** (view-switchable), a **Right Panel** (browser/info/detail), and a **Bottom Panel** (mixer/sends/FX chain with mode tabs).

## Layout

```
┌───────────────────────────────────────────────┐
│  Menu + Toolbar                                │
├───────────────────────┬───────────────────────┤
│                       │  Right Panel           │
│   Main Tile           │  (Browser /            │
│   (Arrange /          │   Clip Info /          │
│    Piano Roll /       │   Effect Detail)       │
│    future editors)    │                        │
├───────────────────────┴───────────────────────┤
│  Bottom Panel (Mixer | Sends | FX Chain)       │
├───────────────────────────────────────────────┤
│  Status Bar                                    │
└───────────────────────────────────────────────┘
```

### egui Panel Declaration Order (app_ui.rs)

```rust
toolbar::render(ctx, ...);                          // TopBottomPanel::top (existing)
egui::SidePanel::right("right_panel")               // NEW: right panel
    .resizable(true).default_width(220.0)
    .show(ctx, |ui| right_panel::render(ui, app));
egui::CentralPanel::default().show(ctx, |ui| {      // Main tile (view-switchable)
    match app.main_view {
        MainView::Arrange   => timeline::render(ui, app),
        MainView::PianoRoll => piano_roll::render_panel(ui, app),
    }
});
egui::TopBottomPanel::bottom("bottom_panel")         // NEW: tabbed bottom panel
    .resizable(true).min_height(160.0)
    .show(ctx, |ui| bottom_panel::render(ui, app));
egui::TopBottomPanel::bottom("status_bar")...        // Very bottom (existing)
```

## Section 1: Main Tile View Switcher

### State

```rust
#[derive(Clone, Copy, PartialEq)]
pub enum MainView {
    Arrange,
    PianoRoll,
}

// In HdawApp:
pub main_view: MainView,           // NEW
pub editing_midi_clip_id: Option<Uuid>,  // already exists
pub show_piano_roll: bool,         // REMOVED — replaced by main_view
```

### Transitions

| Trigger | Location | Action |
|---------|----------|--------|
| Double-click MIDI clip | `clips.rs:567` | `app.main_view = MainView::PianoRoll`, `app.editing_midi_clip_id = Some(clip_id)` |
| Escape key | `app_ui.rs` input handler | `app.main_view = MainView::Arrange`, `app.editing_midi_clip_id = None` |
| Toolbar "Arrange" button | `toolbar.rs` | Same as Escape |
| New project / load project | `project_io.rs` | Reset to `MainView::Arrange` |

### Piano Roll Refactor

- Extract `pub fn render_panel(ui: &mut egui::Ui, app: &mut HdawApp)` — renders content directly (no Window)
- Keep existing `render(ctx, app)` as a thin Window wrapper that calls `render_panel` — available for fallback
- Remove `PianoRoll` from `PanelKind` enum in `panels.rs`

## Section 2: Right Panel

**New file**: `src/ui/right_panel.rb` → `src/ui/right_panel.rs`

### Modes

```rust
pub enum RightPanelMode {
    Browser,      // Audio pool / file browser
    ClipInfo,     // Selected clip properties
    EffectDetail, // Selected track's FX chain / instrument
}
```

### Tabs

Tab buttons at the top of the panel. Content below switches based on selected mode.

- **Browser**: Reuses rendering from `audio_pool.rs` — shows imported clips and files
- **ClipInfo**: Shows properties of `app.timeline_state.selected_clip_id` — name, position, length, transpose (future)
- **EffectDetail**: Shows FX chain for `app.selected_track` — reuses `effect_editor.rs` rendering

### State

```rust
// In HdawApp:
pub right_panel_mode: RightPanelMode,  // which tab is active
```

Default width: 220px, resizable, collapsible via `.resizable(true)`.

## Section 3: Bottom Panel

**New file**: `src/ui/bottom_panel.rs` — replaces `mixer_panel.rs`

### Modes

```rust
pub enum BottomPanelMode {
    Mixer,    // Current channel strips (VU + faders + mute/solo + route + sends)
    Sends,    // Per-track send routing overview
    FxChain,  // Selected track's FX chain parameters
}
```

### Tabs

Tab buttons at the top of the panel. Content below switches based on selected mode.

- **Mixer mode**: Identical to current `mixer_panel.rs` — channel strips with VU meters, faders, mute/solo, route dropdown, sends. The existing `draw_channel`, `draw_master`, `draw_vu_meter` functions move here unchanged.
- **Sends mode**: Grid/table showing each track's send levels to each return track
- **FX Chain mode**: Effect chain of `app.selected_track` — reuses rendering from `effect_editor.rs`

### State

```rust
// In HdawApp:
pub bottom_panel_mode: BottomPanelMode,  // which tab is active
```

### Migration

- `MixerPanelState` merges into `BottomPanelState` (or stays as sub-state)
- Existing `mixer_panel::render()` → `bottom_panel::render(ui, app)` for Mixer mode
- File `mixer_panel.rs` deleted after migration
- Panel `PanelKind::Mixer` removed from `panels.rs`

## Section 4: File Changes

| File | Action |
|------|--------|
| `src/app/mod.rs` | Add `MainView`, `RightPanelMode`, `BottomPanelMode` enums + fields; remove `show_piano_roll` |
| `src/ui/app_ui.rs` | Restructure panel layout; main tile dispatches on `main_view`; Escape handler for view switch |
| `src/ui/panels.rs` | Remove `PianoRoll` and `Mixer` from `PanelKind` |
| `src/ui/piano_roll.rs` | Extract `render_panel(ui, app)`; keep `render(ctx, app)` as Window wrapper |
| `src/ui/mixer_panel.rs` | **Deleted** — contents move to `bottom_panel.rs` |
| `src/ui/bottom_panel.rs` | **New** — tabbed bottom panel |
| `src/ui/right_panel.rs` | **New** — tabbed right panel |
| `src/ui/toolbar.rs` | Add "Arrange" view toggle button when `main_view != Arrange` |
| `src/ui/timeline/clips.rs` | `show_piano_roll = true` → `main_view = MainView::PianoRoll` |
| `src/app/project_io.rs` | Reset `main_view` on new/load |

## Section 5: Edge Cases

- **Playhead continuity**: Piano roll draws its own playhead — no interruption when switching views
- **View state preserved**: Timeline scroll/zoom unchanged when switching away and back
- **Empty selection**: Right panel Clip Info / Effect Detail tabs show "Nothing selected" when no clip/track is selected
- **Panel sizes persisted**: Bottom panel height and right panel width stored in `PreferencesState`
