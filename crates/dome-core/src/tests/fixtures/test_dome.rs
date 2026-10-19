use crate::tests::fixtures::window_labels::WindowLabels;

use crate::action::Action;
use crate::config::{Config, Keystroke};
use crate::core::{
    ContainerId, Hub, LimitObservation, MonitorId, PixelRect, PreferredLayouts, ReportedMonitor,
    WindowId, WindowMatcher, WindowMetadata, WindowRestrictions,
};
use crate::tests::fixtures::configuration::config_source;
use crate::tests::fixtures::directory::ConfigDirectory;
use crate::tests::fixtures::effects::RecordingEffects;
use crate::tests::fixtures::keymap::RecordingKeymap;

pub(crate) struct TestDome {
    hub: Hub,
    directory: ConfigDirectory,
    effects: RecordingEffects,
    keymap: RecordingKeymap,
    window_labels: WindowLabels,
}

const PRIMARY_WORK_AREA: PixelRect = PixelRect::new(0, 0, 100, 20);

impl TestDome {
    pub(crate) fn new(config: &str) -> Self {
        Self::boot(config, None, PRIMARY_WORK_AREA)
    }

    pub(crate) fn with_layout(config: &str, layout: &str) -> Self {
        Self::boot(config, Some(layout), PRIMARY_WORK_AREA)
    }

    pub(crate) fn with_primary_work_area(config: &str, work_area: PixelRect) -> Self {
        Self::boot(config, None, work_area)
    }

    fn boot(config: &str, layout: Option<&str>, work_area: PixelRect) -> Self {
        crate::tests::fixtures::logging::initialize();
        let mut keymap = RecordingKeymap::default();
        let crate::tests::fixtures::configuration::Configuration {
            directory,
            runtime,
            config: loaded,
            layouts,
        } = crate::tests::fixtures::configuration::Configuration::new(config, layout, &mut keymap);
        let primary = ReportedMonitor {
            device_name: "primary".to_owned(),
            work_area,
            scale: 1.0,
            cg_display_id: None,
            gdi_device: None,
        };
        let hub = Hub::new(primary, loaded.tiling, layouts, runtime);
        Self {
            hub,
            directory,
            effects: RecordingEffects::default(),
            keymap,
            window_labels: WindowLabels::default(),
        }
    }

    pub(crate) fn open(&mut self, window: TestWindow) -> WindowId {
        let id = self.window_labels.insert(&window.label);
        let metadata = TestMetadata {
            title: Some(window.title),
            process: None,
        };
        self.hub
            .insert_window(id, Box::new(metadata), window.frame, window.restrictions)
            .expect("no ignore rule matches a test window");
        id
    }

    pub(crate) fn run(&mut self, command: &str) {
        let action: Action = command.parse().expect("the test command should parse");
        self.run_action(action);
    }

    pub(crate) fn run_action(&mut self, action: Action) {
        self.hub
            .handle_action(&action, &mut self.effects, &mut self.keymap);
    }

    pub(crate) fn close(&mut self, id: WindowId) {
        self.hub.delete_window(id);
    }

    pub(crate) fn focus(&mut self, id: WindowId) {
        self.hub.set_focus(id);
    }

    pub(crate) fn tab_container(&self, titles: &[&str]) -> ContainerId {
        self.visible_container(titles, true)
    }

    pub(crate) fn column_container(&self, titles: &[&str]) -> ContainerId {
        self.visible_container(titles, false)
    }

    fn visible_container(&self, titles: &[&str], tabbed: bool) -> ContainerId {
        let matches: Vec<_> = self
            .hub
            .get_visible_placements()
            .monitors
            .into_iter()
            .flat_map(|monitor| match monitor.layout {
                crate::core::MonitorLayout::Normal { containers, .. } => containers,
                crate::core::MonitorLayout::Fullscreen(_) => Vec::new(),
            })
            .filter(|container| container.is_tabbed == tabbed && container.titles == titles)
            .map(|container| container.id)
            .collect();
        assert_eq!(
            matches.len(),
            1,
            "expected one visible container with titles {titles:?} and tabbed={tabbed}"
        );
        matches[0]
    }

    pub(crate) fn click_tab(&mut self, container: ContainerId, index: usize) {
        self.hub.focus_tab_index(container, index);
    }

    pub(crate) fn set_constraint(&mut self, id: WindowId, observed: LimitObservation) {
        self.hub.set_window_constraint(id, observed);
    }

    pub(crate) fn press(&mut self, mode: &str, keystroke: &str) {
        let keystroke: Keystroke = keystroke.parse().expect("the test keystroke should parse");
        self.hub
            .run_binding(mode, &keystroke, &mut self.effects, &mut self.keymap);
    }

    pub(crate) fn published_keymaps(&self) -> Vec<String> {
        self.keymap.published.clone()
    }

    pub(crate) fn reload(&mut self, config: &str) -> Option<Box<Config>> {
        let path = self.directory.write("config.lua", &config_source(config));
        let source = std::fs::read_to_string(path).map_err(Into::into);
        let config = self.hub.load_config(source, &mut self.keymap)?;
        self.hub.config_changed(&config);
        Some(config)
    }

    pub(crate) fn apply_layout(&mut self, layout: &str) {
        let path = self.directory.write("layout.lua", layout);
        let layouts = PreferredLayouts::from_lua(path.to_str().expect("UTF-8 path"), layout)
            .expect("the test layout should load");
        self.hub.apply_preferred_layouts(layouts);
    }

    pub(crate) fn apply_preferred_layouts(&mut self, layouts: PreferredLayouts) {
        self.hub.apply_preferred_layouts(layouts);
    }

    pub(crate) fn capture_live_layouts(&self) -> PreferredLayouts {
        self.hub.capture_live_layouts()
    }

    pub(crate) fn save_layout(&mut self) -> String {
        let path = self.directory.path("layout.lua");
        self.hub
            .capture_live_layouts()
            .save(&path)
            .expect("the layout should save");
        std::fs::read_to_string(&path).expect("the saved layout should read")
    }

    pub(crate) fn layout_backup(&self) -> String {
        std::fs::read_to_string(self.directory.path("layout.lua.bak"))
            .expect("save_layout should have copied the previous layout.lua")
    }

    pub(crate) fn set_title(&mut self, id: WindowId, title: &str) {
        self.hub.set_window_title(id, title.to_owned());
    }

    pub(crate) fn move_float(
        &mut self,
        id: WindowId,
        [x, y, width, height]: [i32; 4],
        monitor: MonitorId,
    ) {
        self.hub.update_float_rect(
            id,
            PixelRect::new(x, y, width.try_into().unwrap(), height.try_into().unwrap()),
            monitor,
        );
    }

    pub(crate) fn monitor_names(&self) -> Vec<String> {
        self.hub
            .query_monitors()
            .into_iter()
            .map(|monitor| monitor.unique_name)
            .collect()
    }

    pub(crate) fn query_workspaces(&self) -> Vec<crate::action::WorkspaceInfo> {
        self.hub.query_workspaces()
    }

    pub(crate) fn primary_monitor(&self) -> MonitorId {
        self.hub.primary_monitor()
    }

    pub(crate) fn minimize(&mut self, id: WindowId) {
        self.hub.minimize_window(id);
    }

    pub(crate) fn set_fullscreen(&mut self, id: WindowId, restrictions: WindowRestrictions) {
        self.hub.set_fullscreen(id, restrictions);
    }

    pub(crate) fn unset_fullscreen(&mut self, id: WindowId) {
        self.hub.unset_fullscreen(id);
    }

    pub(crate) fn unminimize(&mut self, id: WindowId) {
        self.hub.unminimize_window(id);
    }

    pub(crate) fn add_monitor(&mut self, monitor: TestMonitor) -> MonitorId {
        self.hub.add_monitor(monitor.reported())
    }

    pub(crate) fn remove_monitor(&mut self, id: MonitorId) {
        self.hub.remove_monitor(id);
    }

    pub(crate) fn update_monitor(
        &mut self,
        id: MonitorId,
        monitor: TestMonitor,
        displaced: Option<MonitorId>,
    ) {
        self.hub.update_monitor(id, monitor.reported(), displaced);
    }

    pub(crate) fn snapshot(&mut self) -> String {
        let effects = std::mem::take(&mut self.effects);
        let modes = std::mem::take(&mut self.keymap.switched);
        crate::tests::fixtures::snapshot::of_hub(&self.hub, &effects, &modes, &self.window_labels)
    }
}

pub(crate) struct TestWindow {
    title: String,
    label: String,
    frame: PixelRect,
    restrictions: WindowRestrictions,
}

impl TestWindow {
    pub(crate) fn label(mut self, label: &str) -> Self {
        self.label = label.to_owned();
        self
    }

    pub(crate) fn restricted(mut self, restrictions: WindowRestrictions) -> Self {
        self.restrictions = restrictions;
        self
    }

    pub(crate) fn frame(mut self, [x, y, width, height]: [i32; 4]) -> Self {
        self.frame = PixelRect::new(x, y, width.try_into().unwrap(), height.try_into().unwrap());
        self
    }
}

pub(crate) fn window(title: &str) -> TestWindow {
    TestWindow {
        title: title.to_owned(),
        label: title.to_owned(),
        frame: PRIMARY_WORK_AREA,
        restrictions: WindowRestrictions::None,
    }
}

pub(crate) struct TestMonitor {
    name: String,
    frame: PixelRect,
    scale: f64,
    cg_display_id: Option<u32>,
    gdi_device: Option<String>,
}

impl TestMonitor {
    pub(crate) fn cg_display_id(mut self, id: u32) -> Self {
        self.cg_display_id = Some(id);
        self
    }

    pub(crate) fn gdi_device(mut self, device: &str) -> Self {
        self.gdi_device = Some(device.to_owned());
        self
    }

    pub(crate) fn scale(mut self, scale: f64) -> Self {
        self.scale = scale;
        self
    }

    fn reported(self) -> ReportedMonitor {
        ReportedMonitor {
            device_name: self.name,
            work_area: self.frame,
            scale: self.scale,
            cg_display_id: self.cg_display_id,
            gdi_device: self.gdi_device,
        }
    }
}

pub(crate) fn monitor(name: &str, [x, y, width, height]: [i32; 4]) -> TestMonitor {
    TestMonitor {
        name: name.to_owned(),
        frame: PixelRect::new(x, y, width.try_into().unwrap(), height.try_into().unwrap()),
        scale: 1.0,
        cg_display_id: None,
        gdi_device: None,
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct TestMetadata {
    pub(crate) title: Option<String>,
    pub(crate) process: Option<String>,
}

impl std::fmt::Display for TestMetadata {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.title.as_deref().unwrap_or(""))
    }
}

impl WindowMetadata for TestMetadata {
    fn app_name(&self) -> Option<String> {
        None
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
        if matcher.app.is_some()
            || matcher.bundle_id.is_some()
            || matcher.class.is_some()
            || matcher.aumid.is_some()
        {
            return false;
        }
        if let Some(pattern) = matcher.process.as_deref()
            && !self
                .process
                .as_deref()
                .is_some_and(|process| crate::core::pattern_matches(pattern, process))
        {
            return false;
        }
        if let Some(pattern) = matcher.title.as_deref()
            && !self
                .title
                .as_deref()
                .is_some_and(|title| crate::core::pattern_matches(pattern, title))
        {
            return false;
        }
        matcher.process.is_some() || matcher.title.is_some()
    }

    fn to_window_matcher(&self) -> WindowMatcher {
        WindowMatcher {
            title: self.title.clone(),
            process: self.process.clone(),
            ..Default::default()
        }
    }
}
