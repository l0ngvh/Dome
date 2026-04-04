use std::collections::HashMap;

#[derive(Debug)]
pub(super) struct Allocator<T: Node> {
    storage: HashMap<T::Id, T>,
    next_id: usize,
    created: Vec<T::Id>,
    deleted: Vec<T::Id>,
}

impl<T: std::fmt::Debug + Node> Allocator<T> {
    pub(super) fn new() -> Self {
        Self {
            storage: HashMap::new(),
            next_id: 0,
            created: Vec::new(),
            deleted: Vec::new(),
        }
    }

    #[tracing::instrument(skip(self))]
    pub(super) fn allocate(&mut self, node: T) -> T::Id {
        let id = T::Id::new(self.next_id);
        self.next_id += 1;
        self.storage.insert(id, node);
        self.created.push(id);
        id
    }

    #[tracing::instrument(skip(self))]
    pub(super) fn delete(&mut self, id: T::Id) {
        if self.storage.remove(&id).is_some() {
            self.deleted.push(id);
        }
    }

    pub(super) fn drain(&mut self) -> (Vec<T::Id>, Vec<T::Id>) {
        (
            std::mem::take(&mut self.created),
            std::mem::take(&mut self.deleted),
        )
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

    pub(super) fn all_active(&self) -> Vec<(T::Id, T)> {
        let mut entries: Vec<_> = self
            .storage
            .iter()
            .map(|(id, node)| (*id, node.clone()))
            .collect();
        entries.sort_by_key(|(id, _)| id.get());
        entries
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
    fn all_active_returns_only_allocated_nodes() {
        let mut allocator = Allocator::new();
        let id0 = allocator.allocate(TestNode { value: 1 });
        allocator.allocate(TestNode { value: 2 });
        allocator.delete(id0);
        allocator.allocate(TestNode { value: 3 });

        let mut active = allocator.all_active();
        active.sort_by_key(|(id, _)| id.get());
        assert_eq!(active.len(), 2);
        assert_eq!(active[0], (TestId::new(1), TestNode { value: 2 }));
        assert_eq!(active[1], (TestId::new(2), TestNode { value: 3 }));
    }

    #[test]
    fn double_delete_is_harmless() {
        let mut allocator = Allocator::new();
        let id0 = allocator.allocate(TestNode { value: 1 });
        allocator.allocate(TestNode { value: 2 });

        allocator.delete(id0);
        allocator.delete(id0);

        let (_, deleted) = allocator.drain();
        assert_eq!(deleted.len(), 1);
    }

    #[test]
    fn drain_returns_created_and_deleted() {
        let mut allocator = Allocator::new();
        let id0 = allocator.allocate(TestNode { value: 1 });
        allocator.allocate(TestNode { value: 2 });
        allocator.delete(id0);

        let (created, deleted) = allocator.drain();
        assert_eq!(created, vec![TestId::new(0), TestId::new(1)]);
        assert_eq!(deleted, vec![TestId::new(0)]);

        let (created, deleted) = allocator.drain();
        assert!(created.is_empty());
        assert!(deleted.is_empty());
    }

    #[test]
    fn all_active_returns_deterministic_order() {
        let mut allocator = Allocator::new();
        let id0 = allocator.allocate(TestNode { value: 10 });
        let id1 = allocator.allocate(TestNode { value: 20 });
        let id2 = allocator.allocate(TestNode { value: 30 });
        let id3 = allocator.allocate(TestNode { value: 40 });
        allocator.delete(id1);
        allocator.delete(id3);

        let active = allocator.all_active();
        assert_eq!(active.len(), 2);
        assert_eq!(active[0].0, id0);
        assert_eq!(active[1].0, id2);

        for _ in 0..10 {
            let again = allocator.all_active();
            assert_eq!(again, active);
        }
    }
}
