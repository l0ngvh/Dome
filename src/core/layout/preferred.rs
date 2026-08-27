use serde::{Deserialize, Deserializer};

use crate::core::PaneDisplay;
use crate::core::matcher::WindowMatcher;

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct PaneConfig {
    pub(crate) display: PaneDisplay,
    pub(crate) children: Vec<WindowMatcher>,
}

impl PaneConfig {
    #[cfg(test)]
    pub(crate) fn tiled(children: Vec<WindowMatcher>) -> Self {
        Self {
            display: PaneDisplay::Tiled,
            children,
        }
    }
}

#[derive(Deserialize)]
struct PaneContainer {
    #[serde(default)]
    display: PaneDisplay,
    #[serde(default)]
    children: Vec<WindowMatcher>,
}

impl From<PaneContainer> for PaneConfig {
    fn from(c: PaneContainer) -> Self {
        PaneConfig {
            display: c.display,
            children: c.children,
        }
    }
}

impl<'de> Deserialize<'de> for PaneConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = PaneConfig;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("an array of window matchers, or a table with display and children")
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                seq: A,
            ) -> Result<Self::Value, A::Error> {
                let children = Vec::deserialize(serde::de::value::SeqAccessDeserializer::new(seq))?;
                Ok(PaneConfig {
                    display: PaneDisplay::Tiled,
                    children,
                })
            }
            fn visit_map<M: serde::de::MapAccess<'de>>(
                self,
                map: M,
            ) -> Result<Self::Value, M::Error> {
                PaneContainer::deserialize(serde::de::value::MapAccessDeserializer::new(map))
                    .map(Into::into)
            }
        }
        deserializer.deserialize_any(Visitor)
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "strategy")]
pub(crate) enum PreferredWorkspace {
    #[serde(rename = "partition_tree")]
    PartitionTree {
        name: String,
        #[serde(default)]
        tree: Option<TreeLayoutNode>,
        #[serde(default)]
        float: Vec<WindowMatcher>,
        #[serde(default)]
        fullscreen: Vec<WindowMatcher>,
    },
    #[serde(rename = "master")]
    Master {
        name: String,
        #[serde(default)]
        master_ratio: Option<f32>,
        #[serde(default)]
        master_count: Option<usize>,
        #[serde(default)]
        master: PaneConfig,
        #[serde(default)]
        secondary: PaneConfig,
        #[serde(default)]
        float: Vec<WindowMatcher>,
        #[serde(default)]
        fullscreen: Vec<WindowMatcher>,
    },
}

impl PreferredWorkspace {
    pub(crate) fn name(&self) -> &str {
        match self {
            PreferredWorkspace::PartitionTree { name, .. }
            | PreferredWorkspace::Master { name, .. } => name,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum TreeLayoutNode {
    Leaf(WindowMatcher),
    Container {
        split: Option<SplitMode>,
        children: Vec<TreeLayoutNode>,
    },
}

impl<'de> Deserialize<'de> for TreeLayoutNode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = TreeLayoutNode;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a window matcher table, an array of children, or a container table with split and children")
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                seq: A,
            ) -> Result<Self::Value, A::Error> {
                let children = Vec::deserialize(serde::de::value::SeqAccessDeserializer::new(seq))?;
                Ok(TreeLayoutNode::Container {
                    split: None,
                    children,
                })
            }
            fn visit_map<M: serde::de::MapAccess<'de>>(
                self,
                map: M,
            ) -> Result<Self::Value, M::Error> {
                use serde::de::Error;
                // A map is a container when it has `split` or `children`, and a
                // leaf matcher otherwise (the key sets do not overlap). Read it
                // once into a struct holding both shapes, then branch on which
                // keys were present.
                #[derive(Deserialize)]
                struct MapNode {
                    #[serde(default)]
                    split: Option<SplitMode>,
                    #[serde(default)]
                    children: Option<Vec<TreeLayoutNode>>,
                    #[serde(flatten)]
                    matcher: WindowMatcher,
                }
                let node = MapNode::deserialize(serde::de::value::MapAccessDeserializer::new(map))?;
                match (node.split, node.children) {
                    (None, None) => Ok(TreeLayoutNode::Leaf(node.matcher)),
                    (split, Some(children)) => Ok(TreeLayoutNode::Container { split, children }),
                    (Some(_), None) => Err(M::Error::missing_field("children")),
                }
            }
        }
        deserializer.deserialize_any(Visitor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SplitMode {
    Horizontal,
    Vertical,
    Tabbed,
}
