#[derive(Debug)]
pub(super) struct Allocator<T: Node> {
    storage: Vec<Option<T>>,
    free_list: Vec<usize>,
}

impl<T: std::fmt::Debug + Node> Allocator<T> {
    pub(super) fn new() -> Self {
        Self {
            storage: Vec::new(),
            free_list: Vec::new(),
        }
    }

    #[tracing::instrument(skip(self))]
    pub(super) fn allocate(&mut self, node: T) -> T::Id {
        if let Some(free) = self.free_list.pop() {
            self.storage[free] = Some(node);
            T::Id::new(free)
        } else {
            let id = self.storage.len();
            self.storage.push(Some(node));
            T::Id::new(id)
        }
    }

    #[tracing::instrument(skip(self))]
    pub(super) fn delete(&mut self, id: T::Id) {
        let idx = id.get();
        if let Some(slot) = self.storage.get_mut(idx)
            && slot.is_some()
        {
            *slot = None;
            self.free_list.push(idx);
        }
    }

    pub(super) fn get(&self, id: T::Id) -> &T {
        self.storage
            .get(id.get())
            // TODO: dump everything here?
            .unwrap_or_else(|| panic!("Node {id:?} not found"))
            .as_ref()
            .unwrap_or_else(|| panic!("Node {id:?} was deleted"))
    }

    pub(super) fn get_mut(&mut self, id: T::Id) -> &mut T {
        self.storage
            .get_mut(id.get())
            .unwrap_or_else(|| panic!("Node {id:?} not found"))
            .as_mut()
            .unwrap_or_else(|| panic!("Node {id:?} was deleted"))
    }

    pub(super) fn all_active(&self) -> Vec<(T::Id, T)> {
        self.storage
            .iter()
            .enumerate()
            .filter_map(|(idx, node)| node.as_ref().map(|n| (T::Id::new(idx), n.clone())))
            .collect()
    }

    pub(super) fn find(&self, f: impl Fn(&T) -> bool) -> Option<T::Id> {
        self.storage
            .iter()
            .position(|node| node.as_ref().is_some_and(&f))
            .map(T::Id::new)
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
    fn delete_and_allocate_reuses_freed_slots() {
        let mut allocator = Allocator::new();
        let id1 = allocator.allocate(TestNode { value: 1 });
        let id2 = allocator.allocate(TestNode { value: 2 });

        allocator.delete(id1);
        let id3 = allocator.allocate(TestNode { value: 3 });

        assert_eq!(id3.get(), 0); // Reuses freed slot
        assert_eq!(allocator.get(id3).value, 3);
        assert_eq!(allocator.get(id2).value, 2);
    }

    #[test]
    fn all_active_returns_only_allocated_nodes() {
        let mut allocator = Allocator::new();
        let id1 = allocator.allocate(TestNode { value: 1 });
        allocator.allocate(TestNode { value: 2 });
        allocator.delete(id1);
        allocator.allocate(TestNode { value: 3 });

        let active = allocator.all_active();
        assert_eq!(active.len(), 2);
        assert_eq!(active[0], (TestId::new(0), TestNode { value: 3 }));
        assert_eq!(active[1], (TestId::new(1), TestNode { value: 2 }));
    }

    #[test]
    fn double_delete_and_allocate_handles_gracefully() {
        let mut allocator = Allocator::new();
        let id1 = allocator.allocate(TestNode { value: 1 });
        let id2 = allocator.allocate(TestNode { value: 2 });

        allocator.delete(id1);
        allocator.delete(id1);

        let id3 = allocator.allocate(TestNode { value: 3 });
        let id4 = allocator.allocate(TestNode { value: 4 });

        assert_eq!(id3.get(), 0); // Reuses first freed slot
        assert_eq!(id4.get(), 2); // New slot since id1 was only freed once
        assert_eq!(allocator.get(id3).value, 3);
        assert_eq!(allocator.get(id2).value, 2);
        assert_eq!(allocator.get(id4).value, 4);
    }
}
