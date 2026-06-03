use crate::app::HdawApp;
use crate::ui::timeline::automation;
use crate::ui::timeline::{AutoDragState, RULER_HEIGHT};
use egui::{pos2, vec2, Rect, Response};

pub fn sync_automation_to_project(app: &mut HdawApp) {
    if let Some(track_idx) = app.selected_track {
        if let Ok(tracks) = app.engine.tracks.lock() {
            if let Some(handle) = tracks.get(track_idx) {
                for (li, lane) in handle.automation_lanes.iter().enumerate() {
                    if lane.dirty {
                        if let Some(track) = app.project.tracks.get_mut(track_idx) {
                            if let Some(pl) = track.automation_lanes.get_mut(li) {
                                pl.points.clone_from(&lane.points);
                            }
                        }
                    }
                }
            }
        }
        if let Ok(mut tracks) = app.engine.tracks.lock() {
            if let Some(handle) = tracks.get_mut(track_idx) {
                for lane in &mut handle.automation_lanes {
                    lane.dirty = false;
                }
            }
        }
    }
}

pub fn handle_automation_interaction(response: &Response, rect: &Rect, app: &mut HdawApp, header_width: f32, track_height: f32) {
    let track_idx = match app.selected_track {
        Some(i) => i,
        None => return,
    };

    let track_y = rect.top() + RULER_HEIGHT + track_idx as f32 * track_height
        + app.timeline_state.scroll_y as f32;
    let lane_rect = Rect::from_min_size(
        pos2(rect.left() + header_width, track_y),
        vec2((rect.width() - header_width).max(0.0), track_height),
    );

    let pos = match response.hover_pos() {
        Some(p) if lane_rect.contains(p) => p,
        _ => return,
    };

    if response.dragged_by(egui::PointerButton::Primary) && app.timeline_state.auto_drag.is_some() {
        let drag_lane = app.timeline_state.auto_drag.as_ref().map(|d| d.lane_index);
        let drag_point = app.timeline_state.auto_drag.as_ref().map(|d| d.point_index);
        if let (Some(li), Some(pi)) = (drag_lane, drag_point) {
            if let Ok(mut tracks) = app.engine.tracks.lock() {
                if let Some(handle) = tracks.get_mut(track_idx) {
                    if let Some(lane) = handle.automation_lanes.get_mut(li) {
                        let value = automation::param_value_from_y(
                            lane.param_id, pos.y, &lane_rect,
                        );
                        if let Some(pt) = lane.points.get_mut(pi) {
                            pt.value = value;
                            lane.dirty = true;
                        }
                    }
                }
            }
        }
        return;
    }

    if response.clicked_by(egui::PointerButton::Primary) && !response.dragged() {
        if let Ok(mut tracks) = app.engine.tracks.lock() {
            if let Some(handle) = tracks.get_mut(track_idx) {
                let sr = app.engine.transport.sample_rate();
                if let Some((li, pi)) = automation::find_point(
                    &handle.automation_lanes, pos, &lane_rect, &app.timeline_state, sr,
                ) {
                    let old_val = handle.automation_lanes[li].points[pi].value;
                    app.timeline_state.auto_drag = Some(AutoDragState {
                        lane_index: li,
                        point_index: pi,
                        old_value: old_val,
                    });
                } else if let Some((li, _t, value)) = automation::find_segment(
                    &handle.automation_lanes, pos, &lane_rect, &app.timeline_state, sr,
                ) {
                    let sr_f = sr as f64;
                    let pps = app.timeline_state.pixels_per_second;
                    let scroll_x = app.timeline_state.scroll_x;
                    let rel_x = pos.x - lane_rect.left();
                    let bpm = app.project.bpm;
                    let time_frames = app.timeline_state.snap_frames_to_grid(
                        ((rel_x as f64 + scroll_x) / pps * sr_f) as u64, sr, bpm, &app.preferences, &app.project.markers);
                    if let Some(lane) = handle.automation_lanes.get_mut(li) {
                        if let Some(pt) = automation::add_point_to_lane(lane, time_frames, value) {
                            app.undo_service.push(crate::app::undo::UndoCommand::AutomationAddPoint {
                                track_index: track_idx,
                                lane_index: li,
                                point: pt,
                            });
                        }
                    }
                }
            }
        }
        return;
    }

    if response.drag_stopped() {
        if let Some(ad) = app.timeline_state.auto_drag.take() {
            if let Ok(tracks) = app.engine.tracks.lock() {
                if let Some(handle) = tracks.get(track_idx) {
                    if let Some(lane) = handle.automation_lanes.get(ad.lane_index) {
                        if let Some(pt) = lane.points.get(ad.point_index) {
                            app.undo_service.push(crate::app::undo::UndoCommand::AutomationMovePoint {
                                track_index: track_idx,
                                lane_index: ad.lane_index,
                                point_index: ad.point_index,
                                old_value: ad.old_value,
                                new_value: pt.value,
                            });
                        }
                    }
                }
            }
        }
        return;
    }

    if response.clicked_by(egui::PointerButton::Secondary) {
        if let Ok(mut tracks) = app.engine.tracks.lock() {
            if let Some(handle) = tracks.get_mut(track_idx) {
                let sr = app.engine.transport.sample_rate();
                if let Some((li, pi)) = automation::find_point(
                    &handle.automation_lanes, pos, &lane_rect, &app.timeline_state, sr,
                ) {
                    let removed = handle.automation_lanes[li].points[pi].clone();
                    if let Some(lane) = handle.automation_lanes.get_mut(li) {
                        automation::remove_point(lane, pi);
                    }
                    app.undo_service.push(crate::app::undo::UndoCommand::AutomationRemovePoint {
                        track_index: track_idx,
                        lane_index: li,
                        point_index: pi,
                        point: removed,
                    });
                }
            }
        }
    }
}
