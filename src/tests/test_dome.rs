use std::collections::BTreeMap;

use crate::action::{Action, Actions, MonitorDetails};
use crate::config::lua::PlatformEffects;
use crate::config::{Config, Keystroke, LuaRuntime, paths};
use crate::core::NodeId;
use crate::core::{
    ContainerId, Hub, LimitObservation, MonitorId, PixelRect, PreferredLayouts, ReportedMonitor,
    WindowId, WindowMatcher, WindowMetadata, WindowRestrictions,
};
use crate::tests::config_directory::ConfigDirectory;
use crate::tests::recording_keymap::RecordingKeymap;

pub(crate) struct TestDome {
    hub: Hub,
    directory: ConfigDirectory,
    effects: RecordingEffects,
    keymap: RecordingKeymap,
    window_labels: BTreeMap<WindowId, String>,
}

#[derive(Default)]
pub(super) struct RecordingEffects {
    pub(super) closed: Vec<WindowId>,
    pub(super) executed: Vec<String>,
    pub(super) exited: bool,
    pub(super) unminimized: Vec<WindowId>,
}

impl PlatformEffects for RecordingEffects {
    fn close(&mut self, id: WindowId) {
        self.closed.push(id);
    }
    fn execute(&mut self, command: &str) {
        self.executed.push(command.to_string());
    }
    fn exit(&mut self) {
        self.exited = true;
    }
    fn unminimize(&mut self, id: WindowId) {
        self.unminimized.push(id);
    }
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
        install_log_filter();
        let directory = ConfigDirectory::create();
        let config_path = directory.write("config.lua", &config_source(config));
        if let Some(layout) = layout {
            directory.write("layout.lua", layout);
        }
        let layout_path = paths::layout_default_path(&config_path);
        let layouts = PreferredLayouts::load_or_default(layout_path.to_str().expect("UTF-8 path"));
        let mut keymap = RecordingKeymap::default();
        let config_path = config_path.to_str().expect("UTF-8 path").to_owned();
        let (runtime, loaded) =
            LuaRuntime::load(config_path, &mut keymap).expect("the test Lua VM should build");
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
            window_labels: BTreeMap::new(),
        }
    }

    pub(crate) fn open(&mut self, window: TestWindow) -> WindowId {
        assert!(
            !self
                .window_labels
                .values()
                .any(|label| label == &window.label),
            "test windows need distinct labels, use window(title).label(label) for duplicate titles"
        );
        let metadata = TestMetadata {
            title: Some(window.title),
            process: None,
        };
        let id = self
            .hub
            .insert_window(Box::new(metadata), window.frame, window.restrictions)
            .expect("no ignore rule matches a test window");
        self.window_labels.insert(id, window.label);
        id
    }

    pub(crate) fn run(&mut self, command: &str) {
        let action: Action = command.parse().expect("the test command should parse");
        self.hub.handle_actions(
            &Actions::new(vec![action]),
            &mut self.effects,
            &mut self.keymap,
        );
    }

    pub(crate) fn run_action(&mut self, action: Action) {
        self.hub.handle_actions(
            &Actions::new(vec![action]),
            &mut self.effects,
            &mut self.keymap,
        );
    }

    pub(crate) fn close(&mut self, id: WindowId) {
        self.hub.delete_window(id);
    }

    pub(crate) fn focus(&mut self, id: WindowId) {
        self.hub.set_focus(id);
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
        self.directory.write("config.lua", &config_source(config));
        self.hub.reload_config(&mut self.keymap)
    }

    pub(crate) fn apply_layout(&mut self, layout: &str) {
        let path = self.directory.write("layout.lua", layout);
        let layouts = PreferredLayouts::load(path.to_str().expect("UTF-8 path"))
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
        self.hub.save_layout(&path).expect("the layout should save");
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
        self.hub
            .update_float_rect(id, PixelRect::new(x, y, width, height), monitor);
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
        let monitors: Vec<(MonitorId, MonitorDetails)> = self
            .hub
            .query_monitors()
            .into_iter()
            .map(|details| {
                let id = self
                    .hub
                    .monitor_id_by_disambiguated_name(&details.unique_name)
                    .expect("queried monitor should resolve by its unique name");
                (id, details)
            })
            .collect();
        super::snapshot::render(
            &self.hub.get_visible_placements(),
            &self.hub.query_workspaces(),
            &monitors,
            &self.hub.minimized_window_entries(),
            &effects,
            &modes,
            &self.window_labels,
        )
    }
}

pub(crate) fn container(number: usize) -> ContainerId {
    ContainerId::new(number)
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
        self.frame = PixelRect::new(x, y, width, height);
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
    scale: f32,
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

    pub(crate) fn scale(mut self, scale: f32) -> Self {
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
        frame: PixelRect::new(x, y, width, height),
        scale: 1.0,
        cg_display_id: None,
        gdi_device: None,
    }
}

const BASE_SETTINGS: &str = "{
    border_size = 1,
    minimum_width = 1,
    minimum_height = 1,
    partition_tree = { tab_bar_height = 2, automatic_tiling = false },
}";

fn config_source(chunk: &str) -> String {
    format!(
        "local test = (function() {chunk}
end)()
local config = {BASE_SETTINGS}
for key, value in pairs(test) do
    if type(value) == 'table' and type(config[key]) == 'table' then
        for inner_key, inner_value in pairs(value) do
            config[key][inner_key] = inner_value
        end
    else
        config[key] = value
    end
end
return config"
    )
}

fn install_log_filter() {
    use tracing_subscriber::EnvFilter;
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"));
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
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
