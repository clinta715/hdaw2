# HDAW Refactoring Plan

This document outlines the proposed strategy to improve the maintainability and scalability of the HDAW codebase by addressing architectural debt.

## 1. Domain Decomposition (`src/app/mod.rs`)
**Objective:** Decompose the `HdawApp` God object into specialized domain services.

- **Phase 1a:** Extract `Undo/Redo` logic into a dedicated `UndoService`.
- **Phase 1b:** Move file I/O and project structure management into a `ProjectService`.
- **Phase 1c:** Create an `AppCoordinator` to mediate interactions between the UI, `AudioEngine`, and domain services, leaving `HdawApp` as a thin composition layer.

## 2. Audio Processing Modularization (`src/audio/process.rs`)
**Objective:** Decouple audio processing concerns to adhere to the Single Responsibility Principle.

- **Phase 2a:** Extract `MidiEventDispatcher` for processing and mapping MIDI clips.
- **Phase 2b:** Extract `AutomationProcessor` to handle parameter modulation.
- **Phase 2c:** Refactor the main audio loop into an orchestrator that calls the dispatcher, processor, and effect chain sequentially.

## 3. UI Component Orchestration (`src/ui/app_ui.rs`)
**Objective:** Transition to a component-based rendering architecture.

- **Phase 3a:** Implement a `UIManager` trait or registry for active panels (Mixer, Audio Pool, Effect Editor, Plugin GUIs).
- **Phase 3b:** Refactor the `render` loop in `app_ui.rs` to iterate over registered panels instead of hardcoding panel calls.

## Phased Approach
1.  **Phased Implementation:** Start with **Phase 1a** (UndoService) as it has the lowest impact on core audio performance.
2.  **Verification:** Each phase must include unit tests for the extracted logic.
3.  **Stability:** All changes will be verified with `cargo check` and existing test suites throughout the refactoring process.
