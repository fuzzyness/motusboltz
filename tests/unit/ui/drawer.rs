//! Drawer shape tests for `src/ui/drawer.rs`.

use super::*;

const CARD_RECT: Rect = Rect {
    x: 12.0,
    y: 12.0,
    w: 340.0,
    h: 876.0,
};
const SLIDER_CARD: Rect = Rect {
    x: 1134.0,
    y: 6.0,
    w: 288.0,
    h: 290.0,
};

/// Every triangle of the fan from the card's middle turns the same way,
/// so the fan covers the shape once, with no folds, hump and all.
#[test]
fn outlines_fill_cleanly_from_the_middle() {
    for (card, edge) in [(CARD_RECT, Edge::Left), (SLIDER_CARD, Edge::Right)] {
        for with_hump in [true, false] {
            let outline = card_outline(card, edge, with_hump);
            let middle = card.center();
            for (index, &point) in outline.iter().enumerate() {
                let next = outline[(index + 1) % outline.len()];
                let turn = (point - middle).perp_dot(next - middle);
                assert!(turn > 0.0, "{edge:?} hump {with_hump}: folds at {point}");
            }
        }
    }
}

/// The hump stands out on the inner side only, by `HUMP_DEPTH`, and covers
/// the hump's rectangle.
#[test]
fn hump_stands_out_on_the_inner_side() {
    for (card, edge) in [(CARD_RECT, Edge::Left), (SLIDER_CARD, Edge::Right)] {
        let outline = card_outline(card, edge, true);
        let low = outline.iter().map(|p| p.x).fold(f32::MAX, f32::min);
        let high = outline.iter().map(|p| p.x).fold(f32::MIN, f32::max);
        let hump = hump(card, edge);
        match edge {
            Edge::Left => {
                assert!((low - card.x).abs() < 1e-3);
                assert!((high - hump.right()).abs() < 1e-3);
            }
            Edge::Right => {
                assert!((low - hump.x).abs() < 1e-3);
                assert!((high - card.right()).abs() < 1e-3);
            }
        }
        assert!(
            outline
                .iter()
                .all(|p| p.y >= card.y - 1e-3 && p.y <= card.bottom() + 1e-3)
        );
        assert!(hump.y > card.y && hump.bottom() < card.bottom());
    }
}

#[test]
fn tabs_hug_their_edges() {
    let width = 1440.0;
    let left = tab(Edge::Left, width, 28.0, 450.0);
    let right = tab(Edge::Right, width, 28.0, 150.0);
    assert_eq!(left.x, 0.0);
    assert!((right.right() - width).abs() < 1e-3);
    assert!((left.center().y - 450.0).abs() < 1e-3);
    assert!((right.center().y - 150.0).abs() < 1e-3);
}

/// The grip sits inside the part of the hump that stands out from the card.
#[test]
fn grips_sit_inside_their_humps() {
    for (card, edge) in [(CARD_RECT, Edge::Left), (SLIDER_CARD, Edge::Right)] {
        let hump = hump(card, edge);
        let grip = grip(hump);
        assert!(grip.x >= hump.x && grip.right() <= hump.right());
        assert!(grip.y >= hump.y && grip.bottom() <= hump.bottom());
        match edge {
            Edge::Left => assert!(grip.x >= card.right()),
            Edge::Right => assert!(grip.right() <= card.x),
        }
    }
}
