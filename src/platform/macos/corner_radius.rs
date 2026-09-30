//! No public macOS API returns a window's corner radius, and from macOS 26 the radius varies per
//! window. macOS 26 draws the corner concentric with the traffic lights, so the close button's
//! frame predicts the radius. Measured on macOS 26.6 against the private SkyLight radius, this
//! rule matched 49 of 52 apps. It predicts 16 for Zoom (real 12) and 12 for WezTerm (real 0,
//! though WezTerm draws its own corner of about 10pt).

use crate::core::{Dimension, Length, Logical};

/// The radius of a window whose close button carries no signal, such as an Electron app that
/// places its own traffic lights.
pub(super) const FALLBACK_CORNER_RADIUS: Length<Logical> = Length::new(16.0);

/// A close button shorter than this sits in a square-cornered mini titlebar. Stickies is the
/// only measured app with one, a 10pt button.
const MINI_TITLEBAR_BUTTON_HEIGHT: f32 = 12.0;

/// A close button shorter than this belongs to an app built against an SDK older than macOS 26.
/// Those apps keep a 14pt button and a 12pt corner.
const LEGACY_SDK_BUTTON_HEIGHT: f32 = 16.0;
const LEGACY_SDK_CORNER_RADIUS: Length<Logical> = Length::new(12.0);

/// AppKit's close button inset from the top-left corner in a bare titlebar, a compact toolbar,
/// and a full toolbar.
const SYSTEM_CLOSE_BUTTON_INSETS: [f32; 3] = [8.0, 12.0, 18.0];

/// Chrome puts its close button at 12, 12.5.
const SYSTEM_INSET_TOLERANCE: f32 = 0.5;

/// Half the height of a macOS 26 close button. A concentric corner's radius is the button's inset
/// plus this.
const CLOSE_BUTTON_HALF_HEIGHT: f32 = 8.0;

/// Guesses a window's corner radius from its close button.
///
/// macOS 26 centers the close button on the center of the corner curve. So for a button that
/// AppKit placed, the radius is the button's inset from the window corner plus half its height.
/// AppKit always puts the button at one of the system insets, the same distance down as across.
/// Any other position means the app drew its own button. Such a window gets the fallback, as does
/// a window with no close button.
///
/// Smaller buttons skip the inset check. A mini titlebar button means square corners. A button
/// from an SDK older than macOS 26 means the legacy radius.
///
/// `window_origin` and `close_button` must be in the same screen coordinates. Native fullscreen
/// windows have square corners. This function does not check for them.
pub(super) fn corner_radius(
    window_origin: (Length<Logical>, Length<Logical>),
    close_button: Option<Dimension<Logical>>,
) -> Length<Logical> {
    let Some(button) = close_button else {
        return FALLBACK_CORNER_RADIUS;
    };
    let height = button.height.logical();
    if height < MINI_TITLEBAR_BUTTON_HEIGHT {
        return Length::ZERO;
    }
    if height < LEGACY_SDK_BUTTON_HEIGHT {
        return LEGACY_SDK_CORNER_RADIUS;
    }
    let x = button.x.logical() - window_origin.0.logical();
    let y = button.y.logical() - window_origin.1.logical();
    // macOS 26 centers the close button on the corner curve's center. That puts the button on
    // the diagonal, with x equal to y. Zoom's button is at 12, 18, off the diagonal. Checking y
    // alone would predict 26 for Zoom instead of its real 12.
    if (x - y).abs() > 1.0 {
        return FALLBACK_CORNER_RADIUS;
    }
    SYSTEM_CLOSE_BUTTON_INSETS
        .into_iter()
        .find(|inset| (y - inset).abs() <= SYSTEM_INSET_TOLERANCE)
        .map_or(FALLBACK_CORNER_RADIUS, |inset| {
            Length::new(inset + CLOSE_BUTTON_HALF_HEIGHT)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const WINDOW_ORIGIN: (Length<Logical>, Length<Logical>) =
        (Length::new(300.0), Length::new(40.0));

    fn button_at(x: f32, y: f32, width: f32, height: f32) -> Option<Dimension<Logical>> {
        Some(Dimension::new(
            Length::new(WINDOW_ORIGIN.0.logical() + x),
            Length::new(WINDOW_ORIGIN.1.logical() + y),
            Length::new(width),
            Length::new(height),
        ))
    }

    #[test]
    fn measured_apps() {
        let cases = [
            ("no close button", None, 16.0),
            ("Stickies", button_at(11.0, 1.0, 10.0, 10.0), 0.0),
            ("Quip, pre-26 SDK", button_at(8.0, 8.0, 12.0, 14.0), 12.0),
            ("Cisco, pre-26 SDK", button_at(8.0, 8.0, 12.0, 14.5), 12.0),
            (
                "Ghostty, bare titlebar",
                button_at(8.0, 8.0, 16.0, 16.0),
                16.0,
            ),
            (
                "Chrome, compact toolbar",
                button_at(12.0, 12.5, 16.0, 16.0),
                20.0,
            ),
            (
                "Finder, full toolbar",
                button_at(18.0, 18.0, 16.0, 16.0),
                26.0,
            ),
            ("Slack, Electron", button_at(11.0, 11.0, 16.0, 16.0), 16.0),
            ("VS Code, Electron", button_at(10.0, 9.0, 16.0, 16.0), 16.0),
            ("Firefox", button_at(11.0, 14.0, 16.0, 16.0), 16.0),
            (
                "Zoom, off the diagonal",
                button_at(12.0, 18.0, 16.0, 16.0),
                16.0,
            ),
        ];
        for (app, close_button, expected) in cases {
            assert_eq!(
                corner_radius(WINDOW_ORIGIN, close_button).logical(),
                expected,
                "{app}"
            );
        }
    }
}
