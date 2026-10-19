use std::iter::Sum;
use std::marker::PhantomData;
use std::ops::{Add, AddAssign, Div, Mul, Sub, SubAssign};

/// DPI-independent points.
pub struct Logical;
/// Device pixels.
pub struct Physical;
#[cfg(target_os = "windows")]
pub type Unit = Physical;
#[cfg(not(target_os = "windows"))]
pub type Unit = Logical;
pub trait UnitKind {
    fn from_logical(logical: f64, scale: f64) -> f64;
}
impl UnitKind for Logical {
    fn from_logical(logical: f64, _scale: f64) -> f64 {
        logical
    }
}
impl UnitKind for Physical {
    fn from_logical(logical: f64, scale: f64) -> f64 {
        logical * scale
    }
}

macro_rules! scalar {
    ($name:ident, $value:ty, $zero:expr) => {
        pub struct $name<U = Unit> {
            v: $value,
            unit: PhantomData<fn() -> U>,
        }
        impl<U> Copy for $name<U> {}
        impl<U> Clone for $name<U> {
            fn clone(&self) -> Self {
                *self
            }
        }
        impl<U> Default for $name<U> {
            fn default() -> Self {
                Self::ZERO
            }
        }
        impl<U> std::fmt::Debug for $name<U> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct(stringify!($name))
                    .field("v", &self.v)
                    .finish()
            }
        }
        impl<U> std::fmt::Display for $name<U> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.v.fmt(f)
            }
        }
        impl<U> PartialEq for $name<U> {
            fn eq(&self, other: &Self) -> bool {
                self.v == other.v
            }
        }
        impl<U> $name<U> {
            pub const ZERO: Self = Self {
                v: $zero,
                unit: PhantomData,
            };
            pub const fn value(self) -> $value {
                self.v
            }
            pub fn max(self, other: Self) -> Self {
                Self::new(self.v.max(other.v))
            }
            pub fn min(self, other: Self) -> Self {
                Self::new(self.v.min(other.v))
            }
            pub fn clamp(self, low: Self, high: Self) -> Self {
                Self::new(self.v.clamp(low.v, high.v))
            }
        }
    };
}
macro_rules! integer {
    ($name:ident, $value:ty) => {
        scalar!($name, $value, 0);
        impl<U> $name<U> {
            pub const fn new(v: $value) -> Self {
                Self {
                    v,
                    unit: PhantomData,
                }
            }
        }
        impl<U> Eq for $name<U> {}
        impl<U> PartialOrd for $name<U> {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }
        impl<U> Ord for $name<U> {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                self.v.cmp(&other.v)
            }
        }
    };
}
integer!(Coordinate, i32);
integer!(Offset, i64);
integer!(Length, u32);
macro_rules! fractional {
    ($name:ident, $integer:ident) => {
        scalar!($name, f64, 0.0);
        impl<U> PartialOrd for $name<U> {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                self.v.partial_cmp(&other.v)
            }
        }
        impl<U> $name<U> {
            pub fn from_pixels(value: $integer<U>) -> Self {
                Self::new(value.v as f64)
            }
        }
        impl $name<Logical> {
            pub fn to_unit(self, scale: f64) -> $name<Unit> {
                $name::new(<Unit as UnitKind>::from_logical(self.v, scale))
            }
            pub fn logical(self) -> f64 {
                self.v
            }
        }
        impl $name<Physical> {
            pub fn to_logical(self, scale: f64) -> $name<Logical> {
                assert!(
                    scale.is_finite() && scale > 0.0,
                    "scale must be finite and positive"
                );
                $name::new(self.v / scale)
            }
        }
    };
}
fractional!(FractionalCoordinate, Coordinate);
fractional!(FractionalOffset, Offset);
fractional!(FractionalLength, Length);
impl<U> FractionalCoordinate<U> {
    pub const fn new(v: f64) -> Self {
        Self {
            v,
            unit: PhantomData,
        }
    }
    pub fn offset(self) -> FractionalOffset<U> {
        FractionalOffset::new(self.v)
    }
}
impl<U> FractionalOffset<U> {
    pub const fn new(v: f64) -> Self {
        Self {
            v,
            unit: PhantomData,
        }
    }
    pub fn to_length(self) -> FractionalLength<U> {
        FractionalLength::new(self.v.max(0.0))
    }
    pub fn coordinate(self) -> FractionalCoordinate<U> {
        FractionalCoordinate::new(self.v)
    }
}
impl<U> FractionalLength<U> {
    pub const fn new(v: f64) -> Self {
        assert!(v >= 0.0, "length must be nonnegative");
        Self {
            v,
            unit: PhantomData,
        }
    }
    pub fn offset(self) -> FractionalOffset<U> {
        FractionalOffset::new(self.v)
    }
    pub fn shrink(self, amount: Self) -> Self {
        (self - amount).to_length()
    }
}
impl<U> Coordinate<U> {
    pub fn offset(self) -> Offset<U> {
        Offset::new(i64::from(self.v))
    }
    pub fn round(value: FractionalCoordinate<U>) -> Self {
        Self::new(value.v.round() as i32)
    }
}
impl<U> Length<U> {
    pub fn round(value: FractionalLength<U>) -> Self {
        Self::new(value.v.round() as u32)
    }
    pub fn offset(self) -> Offset<U> {
        Offset::new(i64::from(self.v))
    }
    pub fn shrink(self, amount: Self) -> Self {
        Self::new(self.v.saturating_sub(amount.v))
    }
}
impl<U> Offset<U> {
    pub fn coordinate(self) -> Coordinate<U> {
        Coordinate::new(i32::try_from(self.v).expect("coordinate out of range"))
    }
    pub fn to_length(self) -> Length<U> {
        Length::new(u32::try_from(self.v.max(0)).expect("length out of range"))
    }
}
macro_rules! arithmetic {
    ($left:ident, $trait:ident, $method:ident, $operator:tt, $right:ident, $output:ident) => {
        impl<U> $trait<$right<U>> for $left<U> {
            type Output = $output<U>;
            fn $method(self, rhs: $right<U>) -> Self::Output { $output::new(self.v $operator rhs.v) }
        }
    };
}
arithmetic!(FractionalCoordinate, Sub, sub, -, FractionalCoordinate, FractionalOffset);
arithmetic!(FractionalCoordinate, Add, add, +, FractionalOffset, FractionalCoordinate);
arithmetic!(FractionalCoordinate, Sub, sub, -, FractionalOffset, FractionalCoordinate);
arithmetic!(FractionalCoordinate, Add, add, +, FractionalLength, FractionalCoordinate);
arithmetic!(FractionalCoordinate, Sub, sub, -, FractionalLength, FractionalCoordinate);
arithmetic!(FractionalLength, Add, add, +, FractionalLength, FractionalLength);
arithmetic!(FractionalLength, Sub, sub, -, FractionalLength, FractionalOffset);
arithmetic!(FractionalOffset, Add, add, +, FractionalOffset, FractionalOffset);
arithmetic!(FractionalOffset, Sub, sub, -, FractionalOffset, FractionalOffset);
arithmetic!(FractionalOffset, Add, add, +, FractionalLength, FractionalOffset);
arithmetic!(FractionalOffset, Sub, sub, -, FractionalLength, FractionalOffset);
arithmetic!(Offset, Add, add, +, Offset, Offset);
arithmetic!(Offset, Sub, sub, -, Offset, Offset);
arithmetic!(Length, Add, add, +, Length, Length);
impl<U> Sub for Length<U> {
    type Output = Offset<U>;
    fn sub(self, rhs: Self) -> Self::Output {
        Offset::new(i64::from(self.v) - i64::from(rhs.v))
    }
}
impl<U> Sub for Coordinate<U> {
    type Output = Offset<U>;
    fn sub(self, rhs: Self) -> Self::Output {
        Offset::new(i64::from(self.v) - i64::from(rhs.v))
    }
}
impl<U> Add<Offset<U>> for Coordinate<U> {
    type Output = Self;
    fn add(self, rhs: Offset<U>) -> Self {
        Self::new(i32::try_from(i64::from(self.v) + rhs.v).expect("coordinate out of range"))
    }
}
impl<U> Sub<Offset<U>> for Coordinate<U> {
    type Output = Self;
    fn sub(self, rhs: Offset<U>) -> Self {
        Self::new(i32::try_from(i64::from(self.v) - rhs.v).expect("coordinate out of range"))
    }
}
impl<U> Add<Length<U>> for Coordinate<U> {
    type Output = Self;
    fn add(self, rhs: Length<U>) -> Self {
        self + rhs.offset()
    }
}
impl<U> Sub<Length<U>> for Coordinate<U> {
    type Output = Self;
    fn sub(self, rhs: Length<U>) -> Self {
        self - rhs.offset()
    }
}
macro_rules! scale {
    ($name:ident, $value:ty) => {
        impl<U> Mul<$value> for $name<U> {
            type Output = Self;
            fn mul(self, rhs: $value) -> Self {
                Self::new(self.v * rhs)
            }
        }
        impl<U> Mul<$name<U>> for $value {
            type Output = $name<U>;
            fn mul(self, rhs: $name<U>) -> Self::Output {
                rhs * self
            }
        }
        impl<U> Div<$value> for $name<U> {
            type Output = Self;
            fn div(self, rhs: $value) -> Self {
                Self::new(self.v / rhs)
            }
        }
    };
}
scale!(FractionalLength, f64);
scale!(FractionalOffset, f64);
scale!(Length, u32);
scale!(Offset, i64);
macro_rules! assign {
    ($left:ident, $right:ident, $trait:ident, $method:ident, $operator:tt) => {
        impl<U> $trait<$right<U>> for $left<U> { fn $method(&mut self, rhs: $right<U>) { *self = *self $operator rhs } }
    };
}
assign!(FractionalLength, FractionalLength, AddAssign, add_assign, +);
assign!(FractionalOffset, FractionalOffset, AddAssign, add_assign, +);
assign!(FractionalOffset, FractionalLength, AddAssign, add_assign, +);
assign!(FractionalOffset, FractionalLength, SubAssign, sub_assign, -);
assign!(FractionalCoordinate, FractionalLength, AddAssign, add_assign, +);
assign!(FractionalCoordinate, FractionalOffset, AddAssign, add_assign, +);
assign!(FractionalCoordinate, FractionalOffset, SubAssign, sub_assign, -);
impl<U> Sum for FractionalLength<U> {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::ZERO, |a, b| a + b)
    }
}
impl<'a, U: 'a> Sum<&'a Self> for FractionalLength<U> {
    fn sum<I: Iterator<Item = &'a Self>>(iter: I) -> Self {
        iter.copied().sum()
    }
}
macro_rules! pair {
    ($name:ident, $component:ident, $first:ident, $second:ident, $value:ty) => {
        pub struct $name<U = Unit> {
            pub $first: $component<U>,
            pub $second: $component<U>,
        }
        impl<U> Copy for $name<U> {}
        impl<U> Clone for $name<U> {
            fn clone(&self) -> Self {
                *self
            }
        }
        impl<U> PartialEq for $name<U> {
            fn eq(&self, other: &Self) -> bool {
                self.$first == other.$first && self.$second == other.$second
            }
        }
        impl<U> std::fmt::Debug for $name<U> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct(stringify!($name))
                    .field(stringify!($first), &self.$first)
                    .field(stringify!($second), &self.$second)
                    .finish()
            }
        }
        impl<U> $name<U> {
            pub const fn new($first: $value, $second: $value) -> Self {
                Self {
                    $first: $component::new($first),
                    $second: $component::new($second),
                }
            }
            pub const fn $first(self) -> $component<U> {
                self.$first
            }
            pub const fn $second(self) -> $component<U> {
                self.$second
            }
        }
    };
}
pair!(Point, Coordinate, x, y, i32);
pair!(Size, Length, width, height, u32);
pair!(FractionalPoint, FractionalCoordinate, x, y, f64);
pair!(FractionalSize, FractionalLength, width, height, f64);
impl<U> Eq for Point<U> {}
impl<U> Eq for Size<U> {}
impl Point<Physical> {
    pub fn to_logical(self, scale: f64) -> Point<Logical> {
        Point {
            x: Coordinate::round(FractionalCoordinate::from_pixels(self.x).to_logical(scale)),
            y: Coordinate::round(FractionalCoordinate::from_pixels(self.y).to_logical(scale)),
        }
    }
}
pub struct Dimension<U = Unit> {
    pub x: FractionalCoordinate<U>,
    pub y: FractionalCoordinate<U>,
    pub width: FractionalLength<U>,
    pub height: FractionalLength<U>,
}
impl<U> Dimension<U> {
    pub const fn new(
        x: FractionalCoordinate<U>,
        y: FractionalCoordinate<U>,
        width: FractionalLength<U>,
        height: FractionalLength<U>,
    ) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
    pub fn position(self) -> FractionalPoint<U> {
        FractionalPoint {
            x: self.x,
            y: self.y,
        }
    }
    pub fn size(self) -> FractionalSize<U> {
        FractionalSize {
            width: self.width,
            height: self.height,
        }
    }
}
impl<U> Default for Dimension<U> {
    fn default() -> Self {
        Self::new(
            FractionalCoordinate::ZERO,
            FractionalCoordinate::ZERO,
            FractionalLength::ZERO,
            FractionalLength::ZERO,
        )
    }
}
pub struct PixelRect<U = Unit> {
    position: Point<U>,
    size: Size<U>,
}
macro_rules! rectangle_traits {
    ($name:ident) => {
        impl<U> Copy for $name<U> {}
        impl<U> Clone for $name<U> {
            fn clone(&self) -> Self {
                *self
            }
        }
        impl<U> PartialEq for $name<U> {
            fn eq(&self, other: &Self) -> bool {
                self.position() == other.position() && self.size() == other.size()
            }
        }
        impl<U> std::fmt::Debug for $name<U> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let position = self.position();
                let size = self.size();
                f.debug_struct(stringify!($name))
                    .field("x", &position.x.v)
                    .field("y", &position.y.v)
                    .field("width", &size.width.v)
                    .field("height", &size.height.v)
                    .finish()
            }
        }
    };
}
rectangle_traits!(Dimension);
rectangle_traits!(PixelRect);
impl<U> Eq for PixelRect<U> {}
impl<U> PixelRect<U> {
    pub const ZERO: Self = Self::new(0, 0, 0, 0);
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            position: Point::new(x, y),
            size: Size::new(width, height),
        }
    }
    pub const fn from_pixels(
        x: Coordinate<U>,
        y: Coordinate<U>,
        width: Length<U>,
        height: Length<U>,
    ) -> Self {
        Self {
            position: Point { x, y },
            size: Size { width, height },
        }
    }
    pub const fn position(self) -> Point<U> {
        self.position
    }
    pub const fn size(self) -> Size<U> {
        self.size
    }
    pub const fn x(self) -> Coordinate<U> {
        self.position.x
    }
    pub const fn y(self) -> Coordinate<U> {
        self.position.y
    }
    pub const fn width(self) -> Length<U> {
        self.size.width
    }
    pub const fn height(self) -> Length<U> {
        self.size.height
    }
    pub fn right(self) -> Coordinate<U> {
        self.x() + self.width()
    }
    pub fn bottom(self) -> Coordinate<U> {
        self.y() + self.height()
    }
    /// Rounding edges before deriving extents keeps adjacent rectangles aligned.
    pub fn from_dimension(dim: Dimension<U>) -> Self {
        Self::from_edges(
            dim.x.v.round() as i32,
            dim.y.v.round() as i32,
            (dim.x + dim.width).v.round() as i32,
            (dim.y + dim.height).v.round() as i32,
        )
    }
    /// The largest integer rectangle contained in the supplied rectangle.
    pub fn from_dimension_inward(dim: Dimension<U>) -> Self {
        Self::from_edges(
            dim.x.v.ceil() as i32,
            dim.y.v.ceil() as i32,
            (dim.x + dim.width).v.floor() as i32,
            (dim.y + dim.height).v.floor() as i32,
        )
    }
    pub const fn is_empty(self) -> bool {
        self.size.width.v == 0 || self.size.height.v == 0
    }

    pub fn inset_by(self, border: Length<U>) -> Self {
        let twice = border.offset() * 2;
        Self::from_pixels(
            self.x() + border,
            self.y() + border,
            (self.width().offset() - twice).to_length(),
            (self.height().offset() - twice).to_length(),
        )
    }

    pub fn to_dimension(self) -> Dimension<U> {
        Dimension::new(
            FractionalCoordinate::from_pixels(self.x()),
            FractionalCoordinate::from_pixels(self.y()),
            FractionalLength::from_pixels(self.width()),
            FractionalLength::from_pixels(self.height()),
        )
    }

    pub fn outset_by(self, border: Length<U>) -> Self {
        let twice = border.offset() * 2;
        Self::from_pixels(
            self.x() - border,
            self.y() - border,
            (self.width().offset() + twice).to_length(),
            (self.height().offset() + twice).to_length(),
        )
    }

    pub fn clip(self, bounds: Self) -> Option<Self> {
        let left = self.x().max(bounds.x());
        let top = self.y().max(bounds.y());
        let right = self.right().min(bounds.right());
        let bottom = self.bottom().min(bounds.bottom());
        if left >= right || top >= bottom {
            return None;
        }
        Some(Self::from_pixels(
            left,
            top,
            (right - left).to_length(),
            (bottom - top).to_length(),
        ))
    }

    fn from_edges(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self::new(
            left,
            top,
            u32::try_from((i64::from(right) - i64::from(left)).max(0)).expect("width out of range"),
            u32::try_from((i64::from(bottom) - i64::from(top)).max(0))
                .expect("height out of range"),
        )
    }
}
impl PixelRect<Physical> {
    pub fn to_logical(self, scale: f64) -> Dimension<Logical> {
        let dim = self.to_dimension();
        Dimension::new(
            dim.x.to_logical(scale),
            dim.y.to_logical(scale),
            dim.width.to_logical(scale),
            dim.height.to_logical(scale),
        )
    }
}
impl crate::config::lua::deserializer::FromLuaValue for Length<Logical> {
    fn from_lua_value(
        value: &mlua::Value,
        context: &mut crate::config::lua::deserializer::LoadContext,
    ) -> mlua::Result<Self> {
        pixels_from_lua_number(f64::from_lua_value(value, context)?)
    }
}
pub(crate) fn pixels_from_lua_number(value: f64) -> mlua::Result<Length<Logical>> {
    if !value.is_finite() || value < 0.0 {
        return Err(mlua::Error::runtime(
            "pixel value must be a finite non-negative number",
        ));
    }
    if value.fract() != 0.0 {
        return Err(mlua::Error::runtime("pixel value must be a whole number"));
    }
    if value > i32::MAX as f64 {
        return Err(mlua::Error::runtime("pixel value is out of range"));
    }
    Ok(Length::new(value as u32))
}

/// Converts layout-space coordinates to screen-absolute. Layout positions are relative to
/// the workspace origin plus the viewport offset, so the monitor origin is what makes them
/// absolute. The origin is added after rounding rather than before, which is exact because
/// it is integral.
pub(crate) fn translate<U>(
    dim: Dimension<U>,
    offset_x: FractionalOffset<U>,
    offset_y: FractionalOffset<U>,
    screen_x: Coordinate<U>,
    screen_y: Coordinate<U>,
) -> PixelRect<U> {
    let local = PixelRect::from_dimension(Dimension::new(
        dim.x - offset_x,
        dim.y - offset_y,
        dim.width,
        dim.height,
    ));
    PixelRect::from_pixels(
        screen_x + local.x().offset(),
        screen_y + local.y().offset(),
        local.width(),
        local.height(),
    )
}

/// Logical pixels to keep clear on each edge of one monitor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ReservedArea {
    pub top: Length<Logical>,
    pub bottom: Length<Logical>,
    pub left: Length<Logical>,
    pub right: Length<Logical>,
}

impl ReservedArea {
    pub const ZERO: Self = Self {
        top: Length::ZERO,
        bottom: Length::ZERO,
        left: Length::ZERO,
        right: Length::ZERO,
    };

    pub fn subtract_from(self, work_area: PixelRect, scale: f64) -> PixelRect {
        let inset = |px: Length<Logical>| FractionalLength::from_pixels(px).to_unit(scale);
        let area = work_area.to_dimension();
        let far_x = area.x + area.width;
        let far_y = area.y + area.height;
        // A near edge stops at the far edge, so a collapsed area stays inside the work area.
        let left = (area.x + inset(self.left)).min(far_x);
        let top = (area.y + inset(self.top)).min(far_y);
        let right = far_x - inset(self.right);
        let bottom = far_y - inset(self.bottom);
        PixelRect::from_dimension_inward(Dimension::new(
            left,
            top,
            (right - left).to_length(),
            (bottom - top).to_length(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subtract_coordinates_keeps_signed_displacements() {
        let start = Coordinate::<Logical>::new(-20);
        let end = Coordinate::new(5);
        assert_eq!((start - end).value(), -25);
        assert_eq!(start + (end - start), end);
        assert_eq!((Length::<Logical>::new(3) - Length::new(9)).value(), -6);
        let fractional_start = FractionalCoordinate::<Logical>::new(-2.5);
        let fractional_end = FractionalCoordinate::new(0.25);
        assert_eq!((fractional_start - fractional_end).value(), -2.75);
        assert_eq!(
            fractional_start + (fractional_end - fractional_start),
            fractional_end
        );
    }

    #[test]
    fn convert_physical_scales_position_and_extent_independently() {
        let physical = PixelRect::<Physical>::new(-9, 3, 11, 7);
        let logical = physical.to_logical(2.0);
        assert_eq!(logical.x.value(), -4.5);
        assert_eq!(logical.y.value(), 1.5);
        assert_eq!(logical.width.value(), 5.5);
        assert_eq!(logical.height.value(), 3.5);
        assert_eq!(
            FractionalLength::<Physical>::new(3.0)
                .to_logical(2.0)
                .value(),
            1.5
        );
        assert_eq!(
            Point::<Physical>::new(-9, 3).to_logical(2.0),
            Point::new(-5, 2)
        );
    }

    #[test]
    fn round_trip_integer_rectangle_beyond_f32_precision_preserves_pixels() {
        let rect = PixelRect::<Logical>::new(16_777_217, -16_777_219, 5, 7);
        assert_eq!(PixelRect::from_dimension(rect.to_dimension()), rect);
        assert_eq!(
            Point::<Physical>::new(16_777_217, -16_777_217).to_logical(2.0),
            Point::new(8_388_609, -8_388_609)
        );
    }

    #[test]
    fn convert_fractional_scale_preserves_half_pixel_rounding_direction() {
        let scale = 2.000_000_01;
        assert_eq!(
            Point::<Physical>::new(1, -1).to_logical(scale),
            Point::new(0, 0)
        );
        assert_eq!(
            Length::round(FractionalLength::<Physical>::new(1.0).to_logical(scale)),
            Length::ZERO
        );
        assert_eq!(
            Point::<Physical>::new(1, -1).to_logical(2.0),
            Point::new(1, -1)
        );
    }

    #[test]
    fn translate_fractional_offset_and_extent_preserves_narrow_pixel_crossing() {
        let position = FractionalPoint::<Logical>::new(100.5, -100.5);
        let size = FractionalSize::new(0.000_000_02, 0.000_000_02);
        let dim = Dimension::new(position.x, position.y, size.width, size.height);
        assert_eq!(
            translate(
                dim,
                FractionalOffset::new(0.000_000_01),
                FractionalOffset::new(0.000_000_01),
                Coordinate::ZERO,
                Coordinate::ZERO,
            ),
            PixelRect::new(100, -101, 1, 1)
        );
    }

    #[test]
    fn round_inward_keeps_fractional_edges_inside_integer_boundaries() {
        let dim = Dimension::<Logical>::new(
            FractionalCoordinate::new(100.000_000_01),
            FractionalCoordinate::new(-100.999_999_99),
            FractionalLength::new(2.999_999_98),
            FractionalLength::new(2.999_999_98),
        );
        assert_eq!(
            PixelRect::from_dimension_inward(dim),
            PixelRect::new(101, -100, 1, 1)
        );
    }

    #[test]
    fn round_rectangle_uses_edges_not_rounded_extents() {
        let first = Dimension::<Logical>::new(
            FractionalCoordinate::new(-1.4),
            FractionalCoordinate::new(0.4),
            FractionalLength::new(2.8),
            FractionalLength::new(2.2),
        );
        let second = Dimension::new(
            first.x + first.width,
            first.y,
            FractionalLength::new(2.2),
            first.height,
        );
        let rounded = PixelRect::from_dimension(first);
        assert_eq!(rounded, PixelRect::new(-1, 0, 2, 3));
        assert_eq!(rounded.right(), PixelRect::from_dimension(second).x());
        assert_eq!(
            PixelRect::from_dimension_inward(first),
            PixelRect::new(-1, 1, 2, 1)
        );
    }

    #[test]
    fn shrink_past_available_size_clamps_extents_to_zero() {
        let thin = Dimension::<Logical>::new(
            FractionalCoordinate::new(-0.8),
            FractionalCoordinate::new(0.2),
            FractionalLength::new(0.2),
            FractionalLength::new(0.2),
        );
        assert!(PixelRect::from_dimension_inward(thin).is_empty());
        let rect = PixelRect::<Logical>::new(-4, 6, 5, 3);
        assert_eq!(rect.inset_by(Length::new(4)), PixelRect::new(0, 10, 0, 0));
        assert_eq!(
            Length::<Logical>::new(3).shrink(Length::new(9)),
            Length::ZERO
        );
        assert_eq!(
            FractionalLength::<Logical>::new(3.5).shrink(FractionalLength::new(9.0)),
            FractionalLength::ZERO
        );
        assert_eq!(
            rect.outset_by(Length::new(2)).inset_by(Length::new(2)),
            rect
        );
    }

    #[test]
    fn clip_returns_only_nonempty_intersections() {
        let rect = PixelRect::<Logical>::new(-10, -8, 20, 16);
        assert_eq!(
            rect.clip(PixelRect::new(-4, -20, 30, 20)),
            Some(PixelRect::new(-4, -8, 14, 8))
        );
        assert_eq!(rect.clip(PixelRect::new(10, -8, 3, 3)), None);
        assert_eq!(rect.clip(PixelRect::new(-5, -5, 0, 4)), None);
    }

    #[test]
    fn translate_rounds_local_edges_before_adding_monitor_origin() {
        let dim = Dimension::<Logical>::new(
            FractionalCoordinate::new(0.5),
            FractionalCoordinate::new(0.5),
            FractionalLength::new(2.0),
            FractionalLength::new(3.0),
        );
        assert_eq!(
            translate(
                dim,
                FractionalOffset::ZERO,
                FractionalOffset::ZERO,
                Coordinate::new(-10),
                Coordinate::new(-20)
            ),
            PixelRect::new(-9, -19, 2, 3)
        );
        assert_eq!(
            translate(
                dim,
                FractionalOffset::new(2.0),
                FractionalOffset::new(3.0),
                Coordinate::new(-10),
                Coordinate::new(-20)
            ),
            PixelRect::new(-12, -23, 3, 4)
        );
    }

    #[test]
    fn subtract_coordinates_across_full_integer_range_preserves_displacement() {
        let rect = PixelRect::<Logical>::new(i32::MIN, 0, u32::MAX, 1);
        assert_eq!(rect.right().value(), i32::MAX);
        assert_eq!((rect.right() - rect.x()).value(), i64::from(u32::MAX));
        assert_eq!(
            rect.clip(PixelRect::new(-1, 0, 2, 1)),
            Some(PixelRect::new(-1, 0, 2, 1))
        );
    }

    #[test]
    #[should_panic(expected = "length must be nonnegative")]
    fn construct_fractional_length_rejects_negative_magnitudes() {
        let _ = FractionalLength::<Logical>::new(-0.5);
    }
}
