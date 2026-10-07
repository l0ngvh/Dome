//! Pure rectangle math for the occlusion spike. No macOS types here, so it is
//! testable on its own.

/// An axis-aligned rectangle. The spike uses this in two coordinate systems:
/// CGWindowList's global display space (origin top-left, y grows down), and the
/// overlay view's space (origin bottom-left, y grows up). The math is identical
/// in both, because it only ever compares edges.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }

    pub fn right(&self) -> f64 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f64 {
        self.y + self.h
    }

    pub fn is_empty(&self) -> bool {
        self.w <= 0.0 || self.h <= 0.0
    }

    pub fn overlaps(&self, other: &Rect) -> bool {
        self.x < other.right()
            && other.x < self.right()
            && self.y < other.bottom()
            && other.y < self.bottom()
    }

    /// Grow the rectangle by `t` on every side.
    pub fn outset(&self, t: f64) -> Rect {
        Rect::new(self.x - t, self.y - t, self.w + 2.0 * t, self.h + 2.0 * t)
    }

    /// The parts of `self` that `cutter` does not cover, as up to four
    /// non-overlapping rectangles.
    pub fn subtract(&self, cutter: &Rect) -> Vec<Rect> {
        // The covered band, clamped to self.
        let ix = self.x.max(cutter.x);
        let iy = self.y.max(cutter.y);
        let ir = self.right().min(cutter.right());
        let ib = self.bottom().min(cutter.bottom());

        // No real overlap. Nothing is removed.
        if ir <= ix || ib <= iy {
            return vec![*self];
        }

        let mut out = Vec::new();
        // Strip above the covered band.
        if iy > self.y {
            out.push(Rect::new(self.x, self.y, self.w, iy - self.y));
        }
        // Strip below the covered band.
        if ib < self.bottom() {
            out.push(Rect::new(self.x, ib, self.w, self.bottom() - ib));
        }
        // Strip left of the covered band, within the band's height.
        if ix > self.x {
            out.push(Rect::new(self.x, iy, ix - self.x, ib - iy));
        }
        // Strip right of the covered band, within the band's height.
        if ir < self.right() {
            out.push(Rect::new(ir, iy, self.right() - ir, ib - iy));
        }
        out
    }
}

/// A border ring drawn just outside `frame`, of thickness `t`, as its four
/// edge rectangles in the same coordinate system as `frame`.
pub fn border_ring(frame: &Rect, t: f64) -> [Rect; 4] {
    [
        // Top edge, above the frame.
        Rect::new(frame.x - t, frame.y - t, frame.w + 2.0 * t, t),
        // Bottom edge, below the frame.
        Rect::new(frame.x - t, frame.bottom(), frame.w + 2.0 * t, t),
        // Left edge, beside the frame.
        Rect::new(frame.x - t, frame.y, t, frame.h),
        // Right edge, beside the frame.
        Rect::new(frame.right(), frame.y, t, frame.h),
    ]
}

/// Remove every occluder from a set of rectangles. Each occluder splits every
/// surviving piece it overlaps.
pub fn subtract_all(pieces: Vec<Rect>, occluders: &[Rect]) -> Vec<Rect> {
    let mut survivors = pieces;
    for occluder in occluders {
        let mut next = Vec::with_capacity(survivors.len());
        for piece in &survivors {
            next.extend(piece.subtract(occluder).into_iter().filter(|r| !r.is_empty()));
        }
        survivors = next;
        if survivors.is_empty() {
            break;
        }
    }
    survivors
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(rects: &[Rect]) -> f64 {
        rects.iter().map(|r| r.w * r.h).sum()
    }

    #[test]
    fn no_overlap_keeps_whole() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(100.0, 100.0, 10.0, 10.0);
        assert_eq!(a.subtract(&b), vec![a]);
    }

    #[test]
    fn full_cover_removes_all() {
        let a = Rect::new(10.0, 10.0, 10.0, 10.0);
        let b = Rect::new(0.0, 0.0, 100.0, 100.0);
        assert!(a.subtract(&b).is_empty());
    }

    #[test]
    fn hole_in_the_middle_makes_four_pieces() {
        let a = Rect::new(0.0, 0.0, 30.0, 30.0);
        let hole = Rect::new(10.0, 10.0, 10.0, 10.0);
        let pieces = a.subtract(&hole);
        assert_eq!(pieces.len(), 4, "a cut fully inside leaves four edge strips");
        assert!(
            (area(&pieces) - (900.0 - 100.0)).abs() < 1e-9,
            "surviving area is the original minus the cut",
        );
        assert!(
            pieces.iter().all(|p| !p.overlaps(&hole)),
            "no survivor reaches back into the cut",
        );
    }

    #[test]
    fn edge_cover_leaves_an_l_or_strip() {
        // cutter covers the whole left half of a.
        let a = Rect::new(0.0, 0.0, 20.0, 20.0);
        let cutter = Rect::new(0.0, 0.0, 10.0, 20.0);
        let pieces = a.subtract(&cutter);
        assert_eq!(pieces, vec![Rect::new(10.0, 0.0, 10.0, 20.0)]);
    }

    #[test]
    fn overflowing_neighbour_erases_the_covered_border() {
        // A window, a ring around it, and a neighbour that has overflowed its
        // tile far enough to cover the window's right edge.
        let window = Rect::new(100.0, 100.0, 200.0, 200.0);
        let ring = border_ring(&window, 6.0);
        let neighbour = Rect::new(290.0, 90.0, 200.0, 220.0);
        let visible = subtract_all(ring.to_vec(), &[neighbour]);
        assert!(
            visible.iter().all(|r| r.x < window.right()),
            "the covered right edge is gone",
        );
        assert!(
            visible.iter().any(|r| (r.x - (window.x - 6.0)).abs() < 1e-9),
            "the far left edge still survives",
        );
    }
}
