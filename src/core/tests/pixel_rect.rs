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
    let left = PixelRect::from_dimension(dim(0.0, 0.0, 100.5, 50.0));
    let right = PixelRect::from_dimension(dim(100.5, 0.0, 100.5, 50.0));

    assert_eq!(left.right(), right.x());
}
