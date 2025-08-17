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


    pub fn insert(&mut self, tick: Tick, id: StructureId) {
        self.entries.insert((tick, id), ());
    }


    pub fn find(&self, id: StructureId) -> Option<Tick> {
        Some(self.entries.iter().find(|x| x.0.1 == id)?.0.0)
    }

    pub fn remove(&mut self, tick: Tick, id: StructureId) {
        self.entries.remove(&(tick, id));
    }
}


#[test]
fn test_work_queue() {
    use crate::{gen_map::KeyGen, structures::{StructureGen, StructureKey}};
    let mut wq = WorkQueue { entries: BTreeMap::new() };

    let k1 = StructureId(KeyGen::new(StructureGen(0), StructureKey(1)));
    let k2 = StructureId(KeyGen::new(StructureGen(0), StructureKey(2)));
    let k3 = StructureId(KeyGen::new(StructureGen(0), StructureKey(3)));
    let k4 = StructureId(KeyGen::new(StructureGen(0), StructureKey(4)));

    wq.insert(Tick::new(10), k1);
    wq.insert(Tick::new(15), k2);
    wq.insert(Tick::new(20), k4);
    wq.insert(Tick::new(20), k3);

    assert_eq!(&*wq.process(Tick::new(9)), &[]);
    assert_eq!(&*wq.process(Tick::new(10)), &[(Tick::new(10), k1)]);
    assert_eq!(&*wq.process(Tick::new(17)), &[(Tick::new(15), k2)]);
    assert_eq!(&*wq.process(Tick::new(25)), &[(Tick::new(20), k3), (Tick::new(20), k4)]);
}



