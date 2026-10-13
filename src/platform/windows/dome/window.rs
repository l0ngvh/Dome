use super::display_from_process;
use crate::core::{WindowMatcher, pattern_matches};

#[derive(Debug, Clone)]
pub(in crate::platform::windows) struct WindowsMetadata {
    pub title: Option<String>,
    pub process: String,
    pub process_path: Option<String>,
    pub class: Option<String>,
    pub aumid: Option<String>,
    pub app_name: Option<String>,
}

impl std::fmt::Display for WindowsMetadata {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.app_name.as_deref() {
            Some(name) => write!(f, "{name} ({})", self.process)?,
            None => write!(f, "{}", self.process)?,
        }
        if let Some(title) = &self.title {
            write!(f, " - {title}")?;
        }
        if let Some(class) = &self.class {
            write!(f, " class={class}")?;
        }
        if let Some(aumid) = &self.aumid {
            write!(f, " aumid={aumid}")?;
        }
        Ok(())
    }
}

impl crate::core::WindowMetadata for WindowsMetadata {
    fn app_name(&self) -> Option<String> {
        self.app_name
            .clone()
            .or_else(|| Some(display_from_process(&self.process)))
    }
    fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }
    fn executable_path(&self) -> Option<String> {
        self.process_path.clone()
    }
    fn set_title(&mut self, title: String) {
        self.title = Some(title);
    }
    fn clone_box(&self) -> Box<dyn crate::core::WindowMetadata> {
        Box::new(self.clone())
    }

    fn matches_window_matcher(&self, matcher: &WindowMatcher) -> bool {
        let app = self.app_name.as_deref();
        let title = self.title.as_deref();
        let class = self.class.as_deref();
        let aumid = self.aumid.as_deref();

        if let Some(p) = matcher.process.as_deref()
            && !pattern_matches(p, &self.process)
        {
            return false;
        }
        if let Some(p) = matcher.title.as_deref()
            && !title.is_some_and(|t| pattern_matches(p, t))
        {
            return false;
        }
        if let Some(p) = matcher.class.as_deref()
            && !class.is_some_and(|c| pattern_matches(p, c))
        {
            return false;
        }
        if let Some(p) = matcher.aumid.as_deref()
            && !aumid.is_some_and(|a| pattern_matches(p, a))
        {
            return false;
        }
        if let Some(p) = matcher.app.as_deref()
            && !app.is_some_and(|a| pattern_matches(p, a))
        {
            return false;
        }
        matcher.app.is_some()
            || matcher.process.is_some()
            || matcher.title.is_some()
            || matcher.class.is_some()
            || matcher.aumid.is_some()
    }

    fn to_window_matcher(&self) -> WindowMatcher {
        WindowMatcher {
            app: self.app_name.clone(),
            process: Some(self.process.clone()),
            title: self.title.clone(),
            class: self.class.clone(),
            aumid: self.aumid.clone(),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::WindowsMetadata;
    use crate::core::{WindowMatcher, WindowMetadata};

    fn notepad() -> WindowsMetadata {
        WindowsMetadata {
            title: Some(String::from("Untitled - Notepad")),
            process: String::from("notepad.exe"),
            process_path: None,
            class: Some(String::from("Notepad")),
            aumid: None,
            app_name: Some(String::from("Notepad")),
        }
    }

    #[test]
    fn format_discovery_uses_retained_process_identity() {
        let window = crate::platform::windows::window_server::DiscoveredWindow {
            key: crate::platform::windows::external::HwndId::test(0),
            frame: crate::core::PixelRect::new(0, 0, 1, 1),
            monitor: 1,
            fullscreen: None,
            process_id: 7321,
            is_maximized: false,
            metadata: notepad(),
            limits: None,
        };
        assert_eq!(
            window.to_string(),
            "[pid=7321|hwnd=0x0] Notepad (notepad.exe) - Untitled - Notepad class=Notepad",
        );
    }

    #[test]
    fn an_app_only_matcher_matches() {
        let matcher = WindowMatcher {
            app: Some(String::from("Notepad")),
            ..WindowMatcher::default()
        };
        assert!(notepad().matches_window_matcher(&matcher));
    }

    #[test]
    fn an_app_only_matcher_rejects_another_app() {
        let matcher = WindowMatcher {
            app: Some(String::from("Calculator")),
            ..WindowMatcher::default()
        };
        assert!(!notepad().matches_window_matcher(&matcher));
    }

    #[test]
    fn an_empty_matcher_matches_nothing() {
        assert!(!notepad().matches_window_matcher(&WindowMatcher::default()));
    }
}
