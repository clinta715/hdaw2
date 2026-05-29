#[test]
fn test_transport_play_stop() {
    use hdaw::audio::transport::Transport;

    let t = Transport::new(44100);
    assert!(!t.is_playing());
    assert_eq!(t.position_seconds(), 0.0);

    t.play();
    assert!(t.is_playing());

    t.advance_frames(44100);
    assert!((t.position_seconds() - 1.0).abs() < 0.001);

    t.stop();
    assert!(!t.is_playing());
    assert_eq!(t.position_seconds(), 0.0);
}

#[test]
fn test_mixer_volume() {
    use hdaw::audio::mixer::MasterBus;

    let master = MasterBus::new();
    assert!((master.get_volume() - 1.0).abs() < 0.001);

    master.set_volume(0.5);
    assert!((master.get_volume() - 0.5).abs() < 0.001);
}

#[test]
fn test_project_creation() {
    use hdaw::project::Project;

    let p = Project::new();
    assert_eq!(p.name, "Untitled");
    assert_eq!(p.bpm, 120.0);
    assert_eq!(p.tracks.len(), 0);
}

#[test]
fn test_automation_interpolation() {
    use hdaw::project::automation::AutomationLane;

    let mut lane = AutomationLane::new(0, "Volume".into());
    lane.add_point(0, 0.0);
    lane.add_point(100, 1.0);

    assert!((lane.get_value_at(0) - 0.0).abs() < 0.001);
    assert!((lane.get_value_at(100) - 1.0).abs() < 0.001);
    assert!((lane.get_value_at(50) - 0.5).abs() < 0.001);
}
