# HDAW — GUI Layout & Visual Details (Brainstorm)

This document serves as a comprehensive reference for porting the HDAW interface from Qt/QML to `egui`. It captures the current aesthetic, functional layout, and specific widget behaviors.

## 1. Global Aesthetic & Theme

### Color Palette (Hex)
- **Primary Backgrounds:**
  - Timeline Area: `#1a1a1a`
  - Mixer Panel: `#121212`
  - Toolbar: `#2a2a2a`
  - Track Headers: `#222222`
  - FX Editor: `#1a1a1a`
- **Functional Colors:**
  - Dividers/Borders: `#333333`
  - Active Selection/Focus: `#64b5f6` (Light Blue)
  - Success/Playback: `#4caf50` (Green)
  - Warning/Record: `#cc3333` (Red)
  - Solo Highlight: `#cccc33` (Yellow)
  - Playhead: `#ff4444` (Red)
- **Text:**
  - Main Text: `#cccccc`
  - Dimmed Labels: `#777777`
  - Timecode/Values: `#8bc34a` (Light Green)

### Typography
- **Main UI:** Sans-serif (Default system font), sizes 7px (labels) to 11px (headers).
- **Timecode:** Monospace (Consolas/Courier), 18px bold.

---

## 2. Main Window Structure

The app uses a **Single Window** layout with docked, toggleable panels.

```
+-----------------------------------------------------------------+
|  Menu Bar (File, Edit, Track, Transport, View)                  |
+-----------------------------------------------------------------+
|  ToolBar (Tools, Snap, Undo/Redo, Transport, Time, Project)     |
+-----------------------------------------------------------------+
| Pool |                                                   |  FX  |
| (L)  |           Timeline / Arrangement Area             | (R)  |
|      |                                                   |      |
+-----------------------------------------------------------------+
|                       Mixer Panel (Bottom)                      |
+-----------------------------------------------------------------+
```

---

## 3. Component Details

### 3.1 ToolBar (Top, 40px Height)
- **Tool Selector:** 
  - `S` (Select): Blue highlight when active.
  - `C` (Cut/Razor): Red highlight when active.
- **Snap Control:** `M` (Magnet) icon. Blue-ish when enabled.
- **Undo/Redo:** Curved arrows (`\u21B6`, `\u21B7`). Dimmed when stack is empty.
- **Transport Group:**
  - `<<` / `>>`: Go to start/end.
  - `Play` (Green text), `Stop` (Red text), `Rec` (Red dot/Rec text), `Loop` (Blue highlight).
- **Time Display:** `00:00.000` (MM:SS.mmm). Green, bold, monospace.
- **Project Settings:**
  - BPM: `BPM 120.0`. Click to edit (Text input).
  - Time Sig: `4 / 4`. Click opens popup for Num/Denom.
- **Panel Toggles (Right side):** `Import`, `Pool`, `Mixer`, `FX`.

### 3.2 Track Headers (Left, 220px Width)
- **Name:** Left-aligned, color-coded by track theme.
- **Output:** Small label (e.g., `=>Master`).
- **Volume:** Horizontal progress bar style. Green fill.
- **Pan:** Horizontal slider with a center "zero" line. Blue handle.
- **Meters:** Dual 6px-wide vertical bars.
- **Buttons:** `R` (Rec Arm), `M` (Mute), `S` (Solo).
- **FX Slots:** Horizontal row of small blue "chips".
- **Automation:** Button at bottom. Shows active param name or `+A`.

### 3.3 Timeline Area
- **Ruler:** 20px height. Major ticks with time labels, minor ticks between.
- **Grid Lines:** Vertical lines synced to BPM/Zoom.
- **Clips:** 
  - Rounded rectangles (`radius: 3`).
  - Waveform image overlay (stretched to fit).
  - Automation curve overlay (bottom 20px of clip).
  - Fade-in/out: Semi-transparent blue overlays on clip edges.
- **Loop Region:** Semi-transparent green rectangle (`opacity: 0.25`).
- **Playhead:** 2px wide red line.

### 3.4 Mixer Panel (Bottom, 220px Height)
- **Strips:** 50px width each.
- **Strip Colors:**
  - Track: Dark Greenish (`#1a2a1a`)
  - Bus: Dark Blueish (`#1a1a2a`)
  - Master: Dark Yellowish (`#2a2a1a`)
- **Vertical Stack (Top to Bottom):**
  - Small FX Chips (2 per row).
  - Strip Name.
  - Dual Peak Meters (Large, 8px width each).
  - Pan Slider (Horizontal).
  - Volume Fader (Vertical). Uses a 66dB range (-60dB to +6dB).
  - Buttons: `M`, `S`, `R` (circular or square).
  - Output Label (Clickable to cycle).

### 3.5 FX Editor (Side Panel, ~250px Width)
- **Title Bar:** "EFFECT" + Active effect name.
- **Chain:** Row of effect name chips. Current is highlighted. `+` button to add.
- **Bypass:** Checkbox.
- **Parameters:**
  - Label (Left), Value Display (Right).
  - Horizontal Slider (Below).
- **Gain Reduction Meter:** (If Compressor active) Green horizontal bar showing reduction.

---

## 4. Universal CLAP & Automation Architecture

To ensure a future-proof and consistent experience, HDAW will adopt a **Plugin-Centric** DSP model.

### 4.1 CLAP-Only Strategy
- **Exclusive Support:** HDAW will target the **CLAP (Clever Audio Plugin)** standard exclusively for external plugins. This avoids the licensing and technical complexity of VST3 while providing superior threading and parameter modulation support.
- **Internal-as-Plugin:** All native effects (EQ, Compressor, etc.) and even the mixer channel strips will be implemented as internal "CLAP-compatible" modules. They will expose their parameters via the CLAP parameter API.

### 4.2 Unified Automation & Envelopes
- **Universal Parameter Interface:** Because everything is a "plugin," the automation system only needs to talk to one interface. Whether it's a native EQ gain or an external synth's filter cutoff, the automation lane treats them identically.
- **Sample-Accurate Modulation:** Leveraging CLAP's support for per-sample parameter changes, HDAW will support ultra-smooth automation and high-speed envelope modulation without aliasing.
- **Modulation Matrix:** Future expansion will allow for internal "Modulators" (LFOs, Envelopes) to be routed to any parameter across the entire project using this unified interface.

---

## 5. Key Behaviors & Interaction

### Volume Mapping
- Uses logarithmic scaling for sliders: `20 * log10(vol)`.
- Fader range: -60dB to +6dB.
- `-inf` displayed if volume is 0.

### Mouse Interactions
- **Timeline Zoom:** Mouse wheel (horizontal zoom).
- **Timeline Scrub:** Click/drag on ruler or empty timeline.
- **Clip Drag:** Left-click drag to move/trim.
- **Right Click:** Remove clip/effect.
- **Shift + Drag:** Fine adjustment for sliders/knobs.

### Keyboard Shortcuts
- `Space`: Toggle Play/Stop.
- `L`: Toggle Loop.
- `Ctrl+Z` / `Ctrl+Y`: Undo/Redo.
- `+` / `-`: Zoom In/Out.
- `Delete`: Delete Selected.
- `P`: Toggle Audio Pool.
