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
    /// Routes a window's metadata through the matcher lists and returns where to
    /// place it and in what mode, or `None` if nothing matches. Precedence is
    /// mode-outer, workspace-inner: every workspace's fullscreen matchers are
    /// visited before any workspace's float matcher, so a workspace-A float
    /// matcher can never beat a workspace-B fullscreen matcher.
    pub(super) fn resolve_matcher(&self, metadata: &dyn WindowMetadata) -> Option<MatcherHit> {
        // Collect ids once: all_active() clones each Workspace (allocator.rs),
        // so calling it per mode pass doubles the clone on the hot insert path.
        let ws_ids: Vec<WorkspaceId> = self
            .access
            .workspaces
            .all_active()
            .into_iter()
            .map(|(id, _)| id)
            .collect();

        for ws_id in &ws_ids {
            let ws = self.access.workspaces.get(*ws_id);
            for id in &ws.fullscreen_matchers {
                if metadata.matches_window_matcher(self.float_fullscreen_matchers.get(*id)) {
                    return Some(MatcherHit {
                        ws_id: Some(*ws_id),
                        mode: WindowMode::Fullscreen,
                        matcher_id: Some(*id),
                    });
                }
            }
        }
        for ws_id in &ws_ids {
            let ws = self.access.workspaces.get(*ws_id);
            for id in &ws.float_matchers {
                if metadata.matches_window_matcher(self.float_fullscreen_matchers.get(*id)) {
                    return Some(MatcherHit {
                        ws_id: Some(*ws_id),
                        mode: WindowMode::Float,
                        matcher_id: Some(*id),
                    });
                }
            }
        }
        for ws_id in &ws_ids {
            if self
                .strategies
                .for_workspace(*ws_id)
                .matches_tiling(*ws_id, metadata)
            {
                return Some(MatcherHit {
                    ws_id: Some(*ws_id),
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
        // Clear the pool so every id is reallocated fresh from the new config.
        for (id, _) in self.float_fullscreen_matchers.all_active() {
            self.float_fullscreen_matchers.delete(id);
        }

        self.global_float_matchers.clear();
        self.global_fullscreen_matchers.clear();
        for (ws_id, _) in self.access.workspaces.all_active() {
            let w = self.access.workspaces.get_mut(ws_id);
            w.float_matchers.clear();
            w.fullscreen_matchers.clear();
        }

        // Clone globals up front. The allocation loop needs `&mut
        // self.float_fullscreen_matchers` while these read `&self.access.layout`,
        // which would otherwise conflict.
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

        // Re-derive each float/fullscreen window's occupy link by re-matching
        // its live metadata against only the new same-mode matchers on its own
        // workspace. A per-workspace hit relinks to that matcher id; a
        // global-only hit and a no-match both leave occupy None, so the export
        // path synthesises a matcher from live metadata.
        let new_occupies: Vec<(WindowId, Option<FloatFullscreenMatcherId>)> = self
            .access
            .windows
            .all_active()
            .into_iter()
            .filter_map(|(win_id, window)| {
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
