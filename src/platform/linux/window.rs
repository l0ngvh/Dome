use crate::config::{WindowMatcher, pattern_matches};
use crate::core::WindowMetadata;

/// Wayland window metadata snapshot.
///
/// Populated from the client's xdg_toplevel state at map time. Wayland
/// clients change title / app_id at runtime; keeping the hub-side copy in
/// sync with those updates is a follow-up.
#[derive(Debug, Clone)]
pub(super) struct WaylandMetadata {
    pub(super) app_id: Option<String>,
    pub(super) title: Option<String>,
}

impl std::fmt::Display for WaylandMetadata {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.app_id.as_deref().unwrap_or("Unknown"))?;
        if let Some(t) = &self.title {
            write!(f, " - {t}")?;
        }
        Ok(())
    }
}

impl WindowMetadata for WaylandMetadata {
    fn icon_key(&self) -> Option<String> {
        self.app_id.clone()
    }
    fn app_name(&self) -> Option<String> {
        self.app_id.clone()
    }
    fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }
    fn set_title(&mut self, title: String) {
        self.title = Some(title);
    }
    fn clone_box(&self) -> Box<dyn WindowMetadata> {
        Box::new(self.clone())
    }

    fn matches_window_matcher(&self, matcher: &WindowMatcher) -> bool {
        let app_id = self.app_id.as_deref();
        let title = self.title.as_deref();

        if let Some(p) = matcher.app_id.as_deref()
            && !app_id.is_some_and(|a| pattern_matches(p, a))
        {
            return false;
        }
        if let Some(p) = matcher.title.as_deref()
            && !title.is_some_and(|t| pattern_matches(p, t))
        {
            return false;
        }
        // bundle_id / process / class / aumid / app are for other platforms.
        // Silently ignored on wayland, matching the macos / windows convention.
        matcher.app_id.is_some() || matcher.title.is_some()
    }

    fn to_window_matcher(&self) -> WindowMatcher {
        WindowMatcher {
            app_id: self.app_id.clone(),
            title: self.title.clone(),
            ..Default::default()
        }
    }
}
