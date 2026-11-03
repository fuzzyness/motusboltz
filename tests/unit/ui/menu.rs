//! Tests for the menu, `src/ui/menu.rs`.

use super::*;

/// The window the tests use.
const SIZE: Vec2 = Vec2::new(1440.0, 900.0);

/// The rows, top to bottom, for the orbit view or first person.
fn items(menu: &Menu, first_person: bool) -> Vec<Item> {
    menu.rows(SIZE.x, SIZE.y, first_person)
        .into_iter()
        .map(|(item, _)| item)
        .collect()
}

/// A click on `item`.
fn click(menu: &mut Menu, item: Item, first_person: bool) -> Option<Outcome> {
    let (_, rect) = menu
        .rows(SIZE.x, SIZE.y, first_person)
        .into_iter()
        .find(|(row, _)| *row == item)
        .unwrap();
    let outcome = menu.handle(rect.center(), (true, true), SIZE, first_person);
    menu.handle(rect.center(), (false, false), SIZE, first_person);
    outcome
}

/// Resume first, then Options, Exit first person only in first person,
/// and Exit simulation at the bottom, set a little apart, only where the
/// simulation can be left; the rows are centered and apart.
#[test]
fn the_menu_runs_resume_to_exit() {
    let mut menu = Menu {
        can_exit: true,
        ..Menu::default()
    };
    assert_eq!(
        items(&menu, false),
        [Item::Resume, Item::Options, Item::ExitSimulation]
    );
    assert_eq!(
        items(&menu, true),
        [
            Item::Resume,
            Item::Options,
            Item::ExitFirstPerson,
            Item::ExitSimulation
        ]
    );
    let rows = menu.rows(SIZE.x, SIZE.y, true);
    for pair in rows.windows(2) {
        assert!(pair[0].1.bottom() < pair[1].1.y);
    }
    let gap = |index: usize| rows[index + 1].1.y - rows[index].1.bottom();
    assert!(gap(2) > gap(1), "Exit simulation stands apart");
    assert!((rows[0].1.center().x - SIZE.x / 2.0).abs() < 1e-4);
    menu.can_exit = false;
    assert_eq!(items(&menu, false), [Item::Resume, Item::Options]);
}

/// Each button asks for its own thing; Options and Back turn the page,
/// which offers Full screen only in the application.
#[test]
fn the_buttons_do_what_they_say() {
    let mut menu = Menu::default();
    menu.open();
    assert_eq!(
        click(&mut menu, Item::ExitFirstPerson, true),
        Some(Outcome::ExitFirstPerson)
    );
    assert_eq!(click(&mut menu, Item::Resume, false), Some(Outcome::Resume));
    menu.can_exit = true;
    assert_eq!(
        click(&mut menu, Item::ExitSimulation, false),
        Some(Outcome::ExitSimulation)
    );
    assert_eq!(click(&mut menu, Item::Options, false), None);
    assert_eq!(
        items(&menu, false),
        [Item::Sensitivity, Item::FullScreen, Item::Back]
    );
    assert_eq!(
        click(&mut menu, Item::FullScreen, false),
        Some(Outcome::FullScreen)
    );
    // The web page switches full screen with a button of its own.
    menu.can_exit = false;
    assert_eq!(items(&menu, false), [Item::Sensitivity, Item::Back]);
    menu.can_exit = true;
    assert_eq!(click(&mut menu, Item::Back, false), None);
    assert_eq!(items(&menu, false)[0], Item::Resume);
}

/// It pops up when opened and shrinks away when closed.
#[test]
fn the_menu_pops_up_and_shrinks_away() {
    let mut menu = Menu::default();
    assert!(!menu.visible());
    menu.open();
    menu.animate(POP_SECONDS / 2.0);
    assert!(menu.visible() && menu.shown < 1.0);
    menu.animate(POP_SECONDS);
    assert_eq!(menu.shown, 1.0);
    menu.close();
    menu.animate(POP_SECONDS / 2.0);
    assert!(menu.visible(), "still shrinking");
    menu.animate(POP_SECONDS);
    assert!(!menu.visible());
}

/// Dragging the slider's knob sets the sensitivity across its range, and
/// it holds the knob past the track's ends.
#[test]
fn the_slider_sets_the_sensitivity() {
    let mut menu = Menu::default();
    menu.open();
    click(&mut menu, Item::Options, false);
    let (_, rect) = menu.rows(SIZE.x, SIZE.y, false)[0];
    let line = track(rect);
    menu.handle(vec2(line.x, rect.center().y), (true, true), SIZE, false);
    assert!((menu.sensitivity - MIN_SENSITIVITY).abs() < 1e-5);
    menu.handle(vec2(line.right() + 500.0, 0.0), (true, false), SIZE, false);
    assert!((menu.sensitivity - MAX_SENSITIVITY).abs() < 1e-5);
    menu.handle(vec2(line.center().x, 0.0), (true, false), SIZE, false);
    let middle = (MIN_SENSITIVITY + MAX_SENSITIVITY) / 2.0;
    assert!((menu.sensitivity - middle).abs() < 1e-4);
    // Let go, the pointer no longer moves it.
    menu.handle(vec2(line.x, 0.0), (false, false), SIZE, false);
    menu.handle(vec2(line.x, 0.0), (false, false), SIZE, false);
    assert!((menu.sensitivity - middle).abs() < 1e-4);
}
