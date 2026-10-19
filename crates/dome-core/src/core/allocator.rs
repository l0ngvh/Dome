use rustc_hash::FxHashMap;

#[derive(Debug)]
pub(super) struct Allocator<T: Node> {
    /// Keyed by ID, not positional like a Vec, so IDs stay stable across deletions.
    /// Keys are internal, so it uses FxHash rather than the default SipHash.
    storage: FxHashMap<T::Id, T>,
    next_id: usize,
}

impl<T: std::fmt::Debug + Node> Allocator<T> {
    pub(super) fn new() -> Self {
        Self {
            storage: FxHashMap::default(),
            next_id: 0,
        }
    }

    #[tracing::instrument(skip(self))]
    pub(super) fn allocate(&mut self, node: T) -> T::Id {
        let id = T::Id::new(self.next_id);
        self.next_id += 1;
        self.storage.insert(id, node);
        id
    }

    #[tracing::instrument(skip(self))]
    pub(super) fn insert(&mut self, id: T::Id, node: T) {
        assert!(
            !self.storage.contains_key(&id),
            "Node {id:?} already exists"
        );
        self.storage.insert(id, node);
    }

    #[tracing::instrument(skip(self))]
    pub(super) fn delete(&mut self, id: T::Id) {
        self.storage.remove(&id);
    }

    pub(super) fn clear(&mut self) {
        self.storage.clear();
    }

    pub(super) fn get(&self, id: T::Id) -> &T {
        self.storage
            .get(&id)
            .unwrap_or_else(|| panic!("Node {id:?} not found or was deleted"))
    }

    pub(super) fn get_mut(&mut self, id: T::Id) -> &mut T {
        self.storage
            .get_mut(&id)
            .unwrap_or_else(|| panic!("Node {id:?} not found or was deleted"))
    }

    pub(super) fn contains(&self, id: T::Id) -> bool {
        self.storage.contains_key(&id)
    }

    /// Sorted so iteration order stays deterministic regardless of the map's hasher.
    pub(super) fn sorted_ids(&self) -> Vec<T::Id> {
        let mut ids: Vec<T::Id> = self.storage.keys().copied().collect();
        ids.sort_by_key(|id| id.get());
        ids
    }

    pub(super) fn find(&self, f: impl Fn(&T) -> bool) -> Option<T::Id> {
        self.storage
            .iter()
            .find(|(_, node)| f(node))
            .map(|(id, _)| *id)
    }
}

pub(super) trait Node: Clone {
    type Id: NodeId + std::fmt::Debug;
}

pub(super) trait NodeId: Copy + Eq + std::hash::Hash {
    fn new(id: usize) -> Self;
    fn get(self) -> usize;
}
