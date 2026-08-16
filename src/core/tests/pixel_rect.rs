use crate::core::node::{Dimension, Length, PixelRect};

fn dim(x: f32, y: f32, width: f32, height: f32) -> Dimension {
    Dimension::new(
        Length::new(x),
        Length::new(y),
        Length::new(width),
        Length::new(height),
    )
}

/// The macOS AX read path compares against a core target for exact equality, so a
/// truncating spelling here would report a correctly placed window as drifted.
#[test]
fn a_fractional_origin_rounds_rather_than_truncating() {
    let r = PixelRect::from_dimension(dim(-1440.5, 100.5, 800.0, 600.0));

    assert_eq!(r.x().value(), -1441);
    assert_eq!(r.y().value(), 101);
}

#[test]
fn adjacent_boxes_still_share_an_edge_after_rounding() {
    let left = dim(10.5, 0.0, 10.5, 5.0);
    let right = dim(21.0, 0.0, 10.0, 5.0);
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
        "rounding the extents instead of the edges opened a seam"
    );
}
