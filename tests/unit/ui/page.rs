//! Tests for `src/ui/page.rs`: what the page's answers mean, and how they
//! read natively, with no page.

use super::*;

/// The page counts the capture 0 free, 1 pending, 2 held; anything else
/// reads as free.
#[test]
fn the_page_counts_the_capture() {
    assert_eq!(Capture::from_page(0), Capture::Free);
    assert_eq!(Capture::from_page(1), Capture::Pending);
    assert_eq!(Capture::from_page(2), Capture::Held);
    assert_eq!(Capture::from_page(7), Capture::Free);
    assert_eq!(Arm::Capture as i32, 1);
    assert_eq!(Arm::FullScreen as i32, 2);
}

/// Natively every capture is held, none is lost, and the simulation keeps
/// track of full screen itself.
#[test]
fn natively_the_simulation_holds_the_mouse() {
    const { assert!(!HOSTED) };
    assert_eq!(capture(), Capture::Held);
    assert!(!take_lost());
    assert_eq!(full_screen(), None);
}
