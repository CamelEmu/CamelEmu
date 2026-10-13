use std::ops::{Index, IndexMut};

use crate::interval::IntervalValue;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SlotId {
    pub index: u32,
}

impl SlotId {
    pub const INVALID_INDEX: u32 = u32::MAX;

    pub const fn new(index: u32) -> Self {
        Self { index }
    }

    pub const fn is_valid(&self) -> bool {
        self.index != Self::INVALID_INDEX
    }
}

impl Default for SlotId {
    fn default() -> Self {
        Self {
            index: Self::INVALID_INDEX,
        }
    }
}

impl IntervalValue for SlotId {
    fn is_identity(&self) -> bool {
        !self.is_valid()
    }
}

pub struct SlotVector<T> {
    values: Vec<Option<T>>,
    free_list: Vec<u32>,
}

impl<T> Default for SlotVector<T> {
    fn default() -> Self {
        Self {
            values: Vec::new(),
            free_list: Vec::new(),
        }
    }
}

impl<T> SlotVector<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, value: T) -> SlotId {
        let index = self.free_value_index();
        self.values[index as usize] = Some(value);
        SlotId::new(index)
    }

    pub fn swap_and_insert(&mut self, existing_id: SlotId, value: T) -> SlotId {
        let index = self.free_value_index();
        let existing_value = self.values[existing_id.index as usize].replace(value);
        self.values[index as usize] = existing_value;
        SlotId::new(index)
    }

    pub fn erase(&mut self, id: SlotId) {
        self.values[id.index as usize] = None;
        self.free_list.push(id.index);
    }

    pub fn size(&self) -> usize {
        self.values.len() - self.free_list.len()
    }

    /// Returns the value, or `None` when the slot is free.
    pub fn get(&self, id: SlotId) -> Option<&T> {
        self.values.get(id.index as usize)?.as_ref()
    }

    /// Returns the two distinct values at once.
    pub fn get_pair_mut(&mut self, first: SlotId, second: SlotId) -> (&mut T, &mut T) {
        assert!(first != second, "the slots are distinct");
        let (low, high, swapped) = if first.index < second.index {
            (first.index as usize, second.index as usize, false)
        } else {
            (second.index as usize, first.index as usize, true)
        };
        let (head, tail) = self.values.split_at_mut(high);
        let low_value = head[low].as_mut().expect("the slot holds a value");
        let high_value = tail[0].as_mut().expect("the slot holds a value");
        if swapped {
            (high_value, low_value)
        } else {
            (low_value, high_value)
        }
    }

    fn free_value_index(&mut self) -> u32 {
        if self.free_list.is_empty() {
            let capacity = self.values.len();
            self.reserve(if capacity != 0 { capacity << 1 } else { 1 });
        }

        self.free_list.pop().expect("the free list has an entry")
    }

    fn reserve(&mut self, new_capacity: usize) {
        let old_capacity = self.values.len();
        self.values.resize_with(new_capacity, || None);
        self.free_list
            .extend(old_capacity as u32..new_capacity as u32);
    }
}

impl<T> Index<SlotId> for SlotVector<T> {
    type Output = T;

    fn index(&self, id: SlotId) -> &T {
        debug_assert!(id.is_valid());
        self.values[id.index as usize]
            .as_ref()
            .expect("the slot holds a value")
    }
}

impl<T> IndexMut<SlotId> for SlotVector<T> {
    fn index_mut(&mut self, id: SlotId) -> &mut T {
        debug_assert!(id.is_valid());
        self.values[id.index as usize]
            .as_mut()
            .expect("the slot holds a value")
    }
}
