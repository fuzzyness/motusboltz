//! Tests for `src/ui/diagnostics.rs`.

use super::*;

#[test]
fn a_window_averages_each_section_per_frame() {
    assert_eq!(rows(), None);
    for _ in 0..WINDOW_FRAMES {
        add(Section::Scene, 0.004);
        add(Section::GraphicsCalls, 0.001);
        add(Section::Overlay, 0.002);
        add(Section::Measuring, 0.0005);
        add(Section::Panel, 0.0006);
        add(Section::PanelPaint, 0.0004);
        add(Section::Controls, 0.0003);
        add(Section::Labels, 0.0002);
        add(Section::CardScene, 0.003);
        add(Section::CardRest, 0.0035);
        add(Section::HandOff, 0.008);
        end_frame();
    }
    let values: Vec<String> = rows()
        .unwrap()
        .into_iter()
        .map(|(_, value)| value)
        .collect();
    assert_eq!(
        values,
        [
            "4.0 ms", "1.0 ms", "2.0 ms", "0.5 ms", "0.6 ms", "0.4 ms", "0.3 ms", "0.2 ms",
            "3.0 ms", "3.5 ms", "8.0 ms"
        ]
    );
}

#[test]
fn number_keys_switch_pieces_off_and_on() {
    assert!(shown(Piece::Sky));
    assert_eq!(switched_off(), "none");
    switch(1);
    switch(4);
    assert!(!shown(Piece::Sky) && !shown(Piece::SoftView) && shown(Piece::Table));
    assert_eq!(switched_off(), "sky, soft view");
    switch(1);
    switch(4);
    switch(0);
    switch(7);
    assert_eq!(switched_off(), "none");
}
