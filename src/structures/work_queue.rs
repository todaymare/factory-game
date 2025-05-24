use std::{collections::BTreeMap, ops::Bound};

use crate::Tick;

use super::StructureId;

pub struct WorkQueue {
    pub entries: BTreeMap<(Tick, StructureId), ()>,
}


impl WorkQueue {
    pub fn new() -> Self { Self { entries: BTreeMap::new() } }


    pub fn process(&mut self, to_tick: Tick) -> Vec<(Tick, StructureId)> {
        let mut result = Vec::new();
        let mut cursor = self.entries.lower_bound_mut(Bound::Unbounded);
        while let Some(((tick, id), ())) = cursor.next() {
            if *tick > to_tick { break; }
            result.push((*tick, *id));
            cursor.remove_prev();
        }

        return result;
    }
}
