use sti::{define_key, println, vec::KVec};

use crate::gen_map::{KGenMap, KeyGen};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Item(u32);


define_key!(pub Gen(u32));
define_key!(pub BeltLineId(u32));
define_key!(pub IntersectionId(u32));


struct Network {
    belts: KGenMap<Gen, BeltLineId, BeltLine>,
    intersection: KGenMap<Gen, IntersectionId, Intersection>,
}


struct Intersection {
    inputs: Vec<IntersectionId>,
    output: BeltLineId,
}


struct BeltLine {
    items: Vec<Option<Item>>,
}


impl Network {
    pub fn insert(&mut self, belt: BeltLine) -> KeyGen<Gen, BeltLineId> {
        self.belts.insert(belt)
    }

}


impl BeltLine {
    pub fn with_size(size: usize) -> Self {
        Self {
            items: vec![None; size],
        }
    }


    ///
    /// Tries to push the item to the beginning
    /// of the belt line. If it can't push the
    /// item because the end of the belt network is
    /// occupied then it returns back `Some(item)`.
    /// If it can push the item then it returns `None`
    /// 
    pub fn push(&mut self, item: Item) -> Option<Item> {
        let len = self.items.len();
        let slot = &mut self.items[len-1];

        if slot.is_none() {
            *slot = Some(item);
            None
        } else {
            Some(item)
        }
    }


    pub fn can_push(&mut self) -> bool {
        let len = self.items.len();
        let slot = &mut self.items[len-1];
        slot.is_none()
    }


    pub fn tick(&mut self) {
        let mut i = 0;

        loop {
            let next = i;

            i += 1;
            let curr = i;

            if curr == self.items.len() {
                break;
            }


            let next_item = self.items[next];
            let curr_item = self.items[curr];

            if next_item.is_none() {
                self.items[curr] = None;
                self.items[next] = curr_item;
            }
        }
    }
}



#[cfg(test)]
mod tests {
    use super::*;

/*
    #[test]
    fn network_modification() {
        let mut network = Networks::new();

        // insert a belt network
        let belt_network = BeltNetwork::with_size(2);
        assert_eq!(belt_network.output, None);

        let id1 = network.insert(belt_network.clone());

        assert_eq!(network.belts[id1], belt_network);
        assert!(network.roots.contains(&id1));


        // insert another belt network
        let belt_network = BeltNetwork::with_size(3);
        assert_eq!(belt_network.output, None);

        let id2 = network.insert(belt_network.clone());
        assert_eq!(network.belts[id2], belt_network);
        assert!(network.roots.contains(&id2));


        // make an intersection
        let intersection = Intersection::new(vec![id1], id2);
        let idi = network.create_intersection(intersection);

        network.set_output_of(id1, Some(idi));
        assert_eq!(network.belts[id1].output, Some(idi));
        assert!(!network.roots.contains(&id1));
        assert!(network.roots.contains(&id2));

        network.get_belt_mut(id2).input = Some(idi);


        let belt1 = network.get_belt_mut(id1);
        belt1.push(Item(10));
        assert_eq!(network.belts[id1].items, [None, Some(Item(10))]);

        network.tick();
        assert_eq!(network.belts[id1].items, [Some(Item(10)), None]);
        assert_eq!(network.belts[id2].items, [None, None, None]);

        network.tick();
        assert_eq!(network.belts[id1].items, [None, None]);
        assert_eq!(network.belts[id2].items, [None, None, Some(Item(10))]);

        network.tick();
        assert_eq!(network.belts[id1].items, [None, None]);
        assert_eq!(network.belts[id2].items, [None, Some(Item(10)), None]);

        network.tick();
        assert_eq!(network.belts[id1].items, [None, None]);
        assert_eq!(network.belts[id2].items, [Some(Item(10)), None, None]);

    }*/


    #[test]
    fn belt_network_tick() {
        let mut network = BeltLine::with_size(4);
        assert_eq!(network.items, &[None, None, None, None]);

        assert_eq!(network.push(Item(1)), None);
        assert_eq!(network.items, &[None, None, None, Some(Item(1))]);

        network.tick();
        assert_eq!(network.items, &[None, None, Some(Item(1)), None]);

        assert_eq!(network.push(Item(2)), None);
        assert_eq!(network.push(Item(3)), Some(Item(3)));
        assert_eq!(network.items, &[None, None, Some(Item(1)), Some(Item(2))]);

        network.tick();
        assert_eq!(network.items, &[None, Some(Item(1)), Some(Item(2)), None]);

        network.tick();
        assert_eq!(network.items, &[Some(Item(1)), Some(Item(2)), None, None]);
        
        network.tick();
        assert_eq!(network.items, &[Some(Item(1)), Some(Item(2)), None, None]);
        
        assert_eq!(network.push(Item(3)), None);
        assert_eq!(network.items, &[Some(Item(1)), Some(Item(2)), None, Some(Item(3))]);

        network.tick();
        assert_eq!(network.items, &[Some(Item(1)), Some(Item(2)), Some(Item(3)), None]);

        network.tick();
        assert_eq!(network.items, &[Some(Item(1)), Some(Item(2)), Some(Item(3)), None]);

        assert_eq!(network.push(Item(4)), None);
        network.tick();
        assert_eq!(network.items, &[Some(Item(1)), Some(Item(2)), Some(Item(3)), Some(Item(4))]);

        assert_eq!(network.push(Item(5)), Some(Item(5)));
        network.tick();
        assert_eq!(network.items, &[Some(Item(1)), Some(Item(2)), Some(Item(3)), Some(Item(4))]);
        
    }
}
