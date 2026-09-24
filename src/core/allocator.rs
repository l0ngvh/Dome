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
    pub(super) fn delete(&mut self, id: T::Id) {
        self.storage.remove(&id);
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

#[cfg(test)]
mod test {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct TestNode {
        value: i32,
    }

    impl Node for TestNode {
        type Id = TestId;
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    struct TestId(usize);

    impl NodeId for TestId {
        fn new(id: usize) -> Self {
            Self(id)
        }
        fn get(self) -> usize {
            self.0
        }
    }

    #[test]
    fn allocate_assigns_monotonic_ids() {
        let mut allocator = Allocator::new();
        let id0 = allocator.allocate(TestNode { value: 1 });
        let id1 = allocator.allocate(TestNode { value: 2 });
        let id2 = allocator.allocate(TestNode { value: 3 });

        assert_eq!(id0.get(), 0);
        assert_eq!(id1.get(), 1);
        assert_eq!(id2.get(), 2);
    }

    #[test]
    fn delete_and_allocate_does_not_reuse_ids() {
        let mut allocator = Allocator::new();
        let id0 = allocator.allocate(TestNode { value: 1 });
        let id1 = allocator.allocate(TestNode { value: 2 });

        allocator.delete(id0);
        let id2 = allocator.allocate(TestNode { value: 3 });

        assert_eq!(id2.get(), 2);
        assert_eq!(allocator.get(id2).value, 3);
        assert_eq!(allocator.get(id1).value, 2);
    }

    #[test]
    fn sorted_ids_skips_deleted_nodes() {
        let mut allocator = Allocator::new();
        let id0 = allocator.allocate(TestNode { value: 1 });
        allocator.allocate(TestNode { value: 2 });
        allocator.delete(id0);
        allocator.allocate(TestNode { value: 3 });

        let ids = allocator.sorted_ids();

        assert_eq!(ids, vec![TestId::new(1), TestId::new(2)]);
        assert_eq!(allocator.get(ids[0]).value, 2);
        assert_eq!(allocator.get(ids[1]).value, 3);
    }

    #[test]
    fn double_delete_is_harmless() {
        let mut allocator = Allocator::new();
        let id0 = allocator.allocate(TestNode { value: 1 });
        allocator.allocate(TestNode { value: 2 });

        allocator.delete(id0);
        allocator.delete(id0);

        assert_eq!(allocator.sorted_ids().len(), 1);
    }

    #[test]
    fn sorted_ids_returns_ascending_ids() {
        let mut allocator = Allocator::new();
        let id0 = allocator.allocate(TestNode { value: 10 });
        let id1 = allocator.allocate(TestNode { value: 20 });
        let id2 = allocator.allocate(TestNode { value: 30 });
        let id3 = allocator.allocate(TestNode { value: 40 });
        allocator.delete(id1);
        allocator.delete(id3);

        assert_eq!(allocator.sorted_ids(), vec![id0, id2]);
    }
}
