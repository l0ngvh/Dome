use crate::core::node::{Dimension, Length, PixelRect};
use crate::core::strategy::clip;

fn dim(x: f32, y: f32, width: f32, height: f32) -> Dimension {
    Dimension::new(
        Length::new(x),
        Length::new(y),
        Length::new(width),
        Length::new(height),
    )
}

#[test]
fn adjacent_boxes_still_share_an_edge_after_snapping() {
    let left = dim(10.6, 0.0, 10.6, 5.0);
    let right = dim(21.2, 0.0, 10.0, 5.0);
    assert_eq!(
        left.x + left.width,
        right.x,
        "fixture is only meaningful while the two boxes abut exactly"
    );

    let left = PixelRect::from_dimension(left);
    let right = PixelRect::from_dimension(right);

    assert_eq!(
        left.right(),
        right.x(),
        "snapping opened a seam between two boxes that abutted before"
    );
}

#[test]
fn snapping_leaves_an_already_integral_rect_alone() {
    let original = dim(12.0, 34.0, 56.0, 78.0);

    let snapped = PixelRect::from_dimension(original);

    assert_eq!(
        snapped.to_dimension(),
        original,
        "an integral rect must survive the round trip unchanged"
    );
}

/// The two clips are separate because their operands differ, so nothing but a test
/// stops them drifting apart in meaning. Expectations come from the f32 one at
/// runtime rather than from literals.
#[test]
fn the_integer_clip_agrees_with_the_f32_clip_on_integral_input() {
    let bounds = dim(0.0, 0.0, 80.0, 80.0);
    let boxes = [
        dim(10.0, 10.0, 100.0, 50.0),
        dim(-20.0, -5.0, 100.0, 50.0),
        dim(200.0, 200.0, 10.0, 10.0),
        dim(0.0, 0.0, 80.0, 80.0),
        dim(80.0, 0.0, 10.0, 10.0),
        dim(-10.0, -10.0, 5.0, 5.0),
    ];

    for b in boxes {
        assert_eq!(
            PixelRect::from_dimension(b).clip(PixelRect::from_dimension(bounds)),
            clip(b, bounds).map(PixelRect::from_dimension),
            "integer clip disagreed with strategy::clip for {b:?} against {bounds:?}"
        );
    }
}
