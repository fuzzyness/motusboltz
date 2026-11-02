//! Tests for `src/ui/drawer.rs`: the drawers' shapes, slides and peeks.

use super::*;
use crate::ui::pen::grown;

/// The panel's card in a 1440x900 window.
const CARD_RECT: Rect = Rect {
    x: 28.0,
    y: 12.0,
    w: 340.0,
    h: 876.0,
};
/// A card like the sliders' in a 1440-point window, ending where its tab
/// begins.
const SLIDER_CARD: Rect = Rect {
    x: 1120.0,
    y: 6.0,
    w: 292.0,
    h: 290.0,
};

/// Every triangle of the fan from the card's middle turns the same way,
/// so the fan covers the shape once, with no folds, tab and all; the
/// outline stays on the card and its tab.
#[test]
fn outlines_fill_cleanly_from_the_middle() {
    const WIDTH: f32 = 1440.0;
    for (card, edge) in [(CARD_RECT, Edge::Left), (SLIDER_CARD, Edge::Right)] {
        let tab = tab(card, edge, WIDTH);
        for joined in [None, Some(tab)] {
            let outline = card_outline(card, edge, joined);
            let middle = card.center();
            for (index, &point) in outline.iter().enumerate() {
                let next = outline[(index + 1) % outline.len()];
                let turn = (point - middle).perp_dot(next - middle);
                assert!(turn > 0.0, "{edge:?} {joined:?}: folds at {point}");
            }
            let inside =
                |p: &Vec2| grown(card, 1e-3).contains(*p) || grown(tab, FILLET + 1e-3).contains(*p);
            assert!(outline.iter().all(inside));
        }
        // Joined, the outline reaches the window's edge.
        let reach = card_outline(card, edge, Some(tab));
        let low = reach.iter().map(|p| p.x).fold(f32::MAX, f32::min);
        let high = reach.iter().map(|p| p.x).fold(f32::MIN, f32::max);
        match edge {
            Edge::Left => assert!(low.abs() < 1e-3),
            Edge::Right => assert!((high - WIDTH).abs() < 1e-3),
        }
    }
}

/// A drawer's button stands against the window's edge, beside the card's
/// outer side and level with its middle, open or closed, so the pointer
/// that peeks a drawer can keep it open, or close it, where it is.
#[test]
fn buttons_stand_at_the_edge_beside_their_cards() {
    const WIDTH: f32 = 1440.0;
    for (card, edge) in [(CARD_RECT, Edge::Left), (SLIDER_CARD, Edge::Right)] {
        let tab = tab(card, edge, WIDTH);
        assert!((tab.center().y - card.center().y).abs() < 1e-3);
        match edge {
            Edge::Left => assert!(tab.x == 0.0 && tab.right() <= card.x + 1e-3),
            Edge::Right => assert!(tab.right() == WIDTH && tab.x >= card.right() - 1e-3),
        }
        assert!(reach(card, edge, WIDTH).contains(tab.center()));
    }
}

/// One frame at 60 frames per second.
const FRAME: f32 = 1.0 / 60.0;

/// `slide` after `seconds` of frames with the same inputs.
fn run(slide: &mut Slide, seconds: f32, inputs: (bool, bool, bool, bool)) {
    let (open, on_tab, on_card, held) = inputs;
    for _ in 0..(seconds / FRAME).ceil() as usize {
        slide.update(open, on_tab, on_card, held, FRAME);
    }
}

#[test]
fn a_drawer_slides_out_and_back_in_its_time() {
    const DISTANCE: f32 = 300.0;
    let mut slide = Slide::new(false);
    assert_eq!(slide.shift(Edge::Left, DISTANCE), None);
    slide.update(true, false, false, false, FRAME);
    let start = slide.shift(Edge::Left, DISTANCE).unwrap();
    assert!(start < 0.0 && start > -DISTANCE, "toward the left edge");
    // On the way, it never passes its open place.
    let mut furthest = f32::MIN;
    for _ in 0..(SLIDE_SECONDS / FRAME).ceil() as usize {
        slide.update(true, false, false, false, FRAME);
        furthest = furthest.max(slide.shift(Edge::Left, DISTANCE).unwrap());
    }
    assert_eq!(furthest, 0.0);
    assert!(slide.settled_out());
    assert_eq!(slide.shift(Edge::Right, DISTANCE), Some(0.0));
    slide.update(false, false, false, false, FRAME);
    assert!(
        slide.shift(Edge::Right, DISTANCE).unwrap() > 0.0,
        "toward the right edge"
    );
    run(&mut slide, SLIDE_SECONDS, (false, false, false, false));
    assert!(!slide.shown());
}

/// A closed drawer peeks while the pointer rests on its edge button or
/// then on its card, or a press is held, and goes back once neither is
/// true; opening it ends the peek but keeps it out.
#[test]
fn a_peek_lasts_while_the_pointer_stays_on_the_drawer() {
    let mut slide = Slide::new(false);
    run(&mut slide, SLIDE_SECONDS, (false, true, true, false));
    assert!(slide.peeking() && slide.settled_out());
    run(&mut slide, 1.0, (false, false, true, false));
    assert!(slide.peeking(), "still on the card");
    run(&mut slide, 1.0, (false, false, false, true));
    assert!(slide.peeking(), "a drag that left the card");
    slide.update(false, false, false, false, FRAME);
    assert!(!slide.peeking());
    run(&mut slide, SLIDE_SECONDS, (false, false, false, false));
    assert!(!slide.shown());
    // The card alone never starts a peek.
    run(&mut slide, 1.0, (false, false, true, false));
    assert!(!slide.shown());
    run(&mut slide, SLIDE_SECONDS, (false, true, true, false));
    run(&mut slide, 1.0, (true, false, false, false));
    assert!(!slide.peeking() && slide.settled_out());
}

#[test]
fn the_reach_runs_from_the_edge_to_the_cards_inner_side() {
    const WIDTH: f32 = 1440.0;
    let left = reach(CARD_RECT, Edge::Left, WIDTH);
    assert_eq!((left.x, left.right()), (0.0, CARD_RECT.right()));
    let right = reach(SLIDER_CARD, Edge::Right, WIDTH);
    assert_eq!((right.x, right.right()), (SLIDER_CARD.x, WIDTH));
    assert_eq!(slide_distance(SLIDER_CARD, Edge::Right, WIDTH), right.w);
    assert_eq!(slide_distance(CARD_RECT, Edge::Left, WIDTH), left.w);
}

/// The glow's middle runs from the card's outer side, out through the
/// fillets and round the tab, and back to the card's outer side, never
/// along the side the tab shares with the card; its trails run on along
/// the card's outer side, fading, and stop short of the card's corners.
#[test]
fn the_glow_follows_the_joined_outline_and_trails_off() {
    const WIDTH: f32 = 1440.0;
    for (card, edge) in [(CARD_RECT, Edge::Left), (SLIDER_CARD, Edge::Right)] {
        let tab = tab(card, edge, WIDTH);
        let side = match edge {
            Edge::Left => card.x,
            Edge::Right => card.right(),
        };
        let path = glow_path(card, tab, edge);
        let (first, last) = (path[0], path[path.len() - 1]);
        assert!((first - vec2(side, tab.y - FILLET)).length() < 1e-3);
        assert!((last - vec2(side, tab.bottom() + FILLET)).length() < 1e-3);
        let outline = card_outline(card, edge, Some(tab));
        let near = |p: &Vec2| outline.iter().any(|q| (*q - *p).length() < 1.0);
        assert!(path.iter().all(near), "{edge:?} leaves the outline");
        let between =
            |p: &Vec2| tab.y - FILLET - 1e-3 <= p.y && p.y <= tab.bottom() + FILLET + 1e-3;
        assert!(path.iter().all(between));
        let trails = glow_trails(card, tab, edge);
        assert!(trails.len() >= 2 * 8, "pieces each way");
        for &(from, to, fade) in &trails {
            assert!((from.x - side).abs() < 1e-3 && (to.x - side).abs() < 1e-3);
            assert!(fade > 0.0 && fade <= 1.0);
            let inside = card.y + CARD_RADIUS - 1e-3..=card.bottom() - CARD_RADIUS + 1e-3;
            assert!(inside.contains(&from.y) && inside.contains(&to.y));
        }
        let upward: Vec<f32> = trails
            .iter()
            .filter(|t| t.0.y < tab.y)
            .map(|t| t.2)
            .collect();
        assert!(
            upward.windows(2).all(|pair| pair[1] < pair[0]),
            "fades as it goes"
        );
    }
}

/// A closed tab stands out of the window's edge through fillets that
/// reach past its top and bottom along the edge, fans cleanly from the
/// edge, and closes just beyond it.
#[test]
fn a_closed_tab_comes_out_of_the_window_edge() {
    const WIDTH: f32 = 1440.0;
    for (card, edge) in [(CARD_RECT, Edge::Left), (SLIDER_CARD, Edge::Right)] {
        let tab = tab(card, edge, WIDTH);
        let outline = closed_tab_outline(tab, edge, WIDTH);
        let wall = match edge {
            Edge::Left => 0.0,
            Edge::Right => WIDTH,
        };
        let middle = vec2(wall, tab.center().y);
        let turns: Vec<f32> = (0..outline.len())
            .map(|index| {
                let next = outline[(index + 1) % outline.len()];
                (outline[index] - middle).perp_dot(next - middle)
            })
            .filter(|turn| turn.abs() > 1e-3)
            .collect();
        let one_way = turns.iter().all(|&t| t > 0.0) || turns.iter().all(|&t| t < 0.0);
        assert!(one_way, "{edge:?} folds");
        let top = outline.iter().map(|p| p.y).fold(f32::MAX, f32::min);
        let bottom = outline.iter().map(|p| p.y).fold(f32::MIN, f32::max);
        assert!((top - (tab.y - FILLET)).abs() < 1e-3);
        assert!((bottom - (tab.bottom() + FILLET)).abs() < 1e-3);
        let inner = outline
            .iter()
            .map(|p| (p.x - wall).abs())
            .fold(0.0, f32::max);
        assert!((inner - tab.w).abs() < 1e-3, "{edge:?} reaches {inner}");
    }
}
