use crate::config::{LayoutWorkspaceConfig, WindowMatcher, WindowMode};

use super::allocator::{Node, NodeId};
use super::hub::Hub;
use super::node::{DisplayMode, WindowId, WindowMetadata, WorkspaceId};

/// Handle to a matcher in the pool. A window's `DisplayMode` keeps it so the
/// export path can re-find the matcher after the tree has mutated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct FloatFullscreenMatcherId(usize);

impl NodeId for FloatFullscreenMatcherId {
    fn new(id: usize) -> Self {
        Self(id)
    }
    fn get(self) -> usize {
        self.0
    }
}

impl Node for WindowMatcher {
    type Id = FloatFullscreenMatcherId;
}

/// Result of routing a new window through the matcher lists.
pub(super) struct MatcherHit {
    /// Workspace to place the window on. `None` means the current workspace, used by global matchers.
    pub(super) ws_id: Option<WorkspaceId>,
    pub(super) mode: WindowMode,
    /// Links the window back to the matcher that routed it. `None` for tiling hits (tiling has no occupy field) and global hits (export only writes per-workspace matchers, so a global id has no destination).
    pub(super) matcher_id: Option<FloatFullscreenMatcherId>,
}

impl Hub {
    /// Routes a window's metadata to a placement, or `None` if nothing matches.
    pub(super) fn resolve_matcher(&self, metadata: &dyn WindowMetadata) -> Option<MatcherHit> {
        let current_ws = self.current_workspace();
        let search_order: Vec<WorkspaceId> = std::iter::once(current_ws)
            .chain(
                self.access
                    .workspaces
                    .sorted_ids()
                    .into_iter()
                    .filter(|&id| id != current_ws),
            )
            .collect();

        for &ws_id in &search_order {
            let ws = self.access.workspaces.get(ws_id);
            for id in &ws.fullscreen_matchers {
                if metadata.matches_window_matcher(self.float_fullscreen_matchers.get(*id)) {
                    return Some(MatcherHit {
                        ws_id: Some(ws_id),
                        mode: WindowMode::Fullscreen,
                        matcher_id: Some(*id),
                    });
                }
            }
        }
        for &ws_id in &search_order {
            let ws = self.access.workspaces.get(ws_id);
            for id in &ws.float_matchers {
                if metadata.matches_window_matcher(self.float_fullscreen_matchers.get(*id)) {
                    return Some(MatcherHit {
                        ws_id: Some(ws_id),
                        mode: WindowMode::Float,
                        matcher_id: Some(*id),
                    });
                }
            }
        }
        for &ws_id in &search_order {
            if self
                .strategies
                .for_workspace(ws_id)
                .matches_tiling(ws_id, metadata)
            {
                return Some(MatcherHit {
                    ws_id: Some(ws_id),
                    mode: WindowMode::Tiling,
                    matcher_id: None,
                });
            }
        }
        for id in &self.global_fullscreen_matchers {
            if metadata.matches_window_matcher(self.float_fullscreen_matchers.get(*id)) {
                return Some(MatcherHit {
                    ws_id: None,
                    mode: WindowMode::Fullscreen,
                    matcher_id: None,
                });
            }
        }
        for id in &self.global_float_matchers {
            if metadata.matches_window_matcher(self.float_fullscreen_matchers.get(*id)) {
                return Some(MatcherHit {
                    ws_id: None,
                    mode: WindowMode::Float,
                    matcher_id: None,
                });
            }
        }
        None
    }

    /// Rebuilds the matcher pool and every routing vec, per-workspace and
    /// global, from the current config. Runs on both config entry points so
    /// neither per-workspace matchers (via the arg) nor global matchers (via
    /// `self.access.layout`) go stale.
    pub(super) fn index_matchers(&mut self, preferred_layouts: &[LayoutWorkspaceConfig]) {
        for id in self.float_fullscreen_matchers.sorted_ids() {
            self.float_fullscreen_matchers.delete(id);
        }

        self.global_float_matchers.clear();
        self.global_fullscreen_matchers.clear();
        for ws_id in self.access.workspaces.sorted_ids() {
            let w = self.access.workspaces.get_mut(ws_id);
            w.float_matchers.clear();
            w.fullscreen_matchers.clear();
        }

        // Clone globals up front: the allocation loop needs `&mut
        // self.float_fullscreen_matchers` while these borrow `&self.access.layout`.
        let global_fullscreen = self.access.layout.fullscreen.clone();
        let global_float = self.access.layout.float.clone();

        for entry in preferred_layouts {
            let ws_id = self.get_or_create_workspace_on(entry.name(), None);
            let matchers = workspace_matchers(entry);
            for m in matchers.fullscreen {
                let id = self.float_fullscreen_matchers.allocate(m);
                self.access
                    .workspaces
                    .get_mut(ws_id)
                    .fullscreen_matchers
                    .push(id);
            }
            for m in matchers.float {
                let id = self.float_fullscreen_matchers.allocate(m);
                self.access
                    .workspaces
                    .get_mut(ws_id)
                    .float_matchers
                    .push(id);
            }
        }

        for m in global_fullscreen {
            let id = self.float_fullscreen_matchers.allocate(m);
            self.global_fullscreen_matchers.push(id);
        }
        for m in global_float {
            let id = self.float_fullscreen_matchers.allocate(m);
            self.global_float_matchers.push(id);
        }

        // Re-match each float/fullscreen window against only its own workspace's
        // same-mode matchers. A global-only hit or a no-match leaves occupy None,
        // so the export path synthesises a matcher from live metadata.
        let new_occupies: Vec<(WindowId, Option<FloatFullscreenMatcherId>)> = self
            .access
            .windows
            .sorted_ids()
            .into_iter()
            .filter_map(|win_id| {
                let window = self.access.windows.get(win_id);
                let is_float = match window.mode {
                    DisplayMode::Float { .. } => true,
                    DisplayMode::Fullscreen { .. } => false,
                    DisplayMode::Tiling => return None,
                };
                let occupy = window.workspace().and_then(|ws_id| {
                    let ws = self.access.workspaces.get(ws_id);
                    let ids = if is_float {
                        &ws.float_matchers
                    } else {
                        &ws.fullscreen_matchers
                    };
                    ids.iter()
                        .find(|id| {
                            window
                                .metadata
                                .matches_window_matcher(self.float_fullscreen_matchers.get(**id))
                        })
                        .copied()
                });
                Some((win_id, occupy))
            })
            .collect();

        for (win_id, new_occupy) in new_occupies {
            match &mut self.access.windows.get_mut(win_id).mode {
                DisplayMode::Float { occupy, .. } => *occupy = new_occupy,
                DisplayMode::Fullscreen { occupy } => *occupy = new_occupy,
                DisplayMode::Tiling => {}
            }
        }
    }
}

struct Matchers {
    fullscreen: Vec<WindowMatcher>,
    float: Vec<WindowMatcher>,
}

fn workspace_matchers(entry: &LayoutWorkspaceConfig) -> Matchers {
    match entry {
        LayoutWorkspaceConfig::PartitionTree {
            fullscreen, float, ..
        } => Matchers {
            fullscreen: fullscreen.clone(),
            float: float.clone(),
        },
        LayoutWorkspaceConfig::Master {
            fullscreen, float, ..
        } => Matchers {
            fullscreen: fullscreen.clone(),
            float: float.clone(),
        },
    }
}
