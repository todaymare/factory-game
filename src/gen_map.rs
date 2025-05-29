use core::ops::{Index, IndexMut};

use sti::{alloc::{Alloc, GlobalAlloc}, key::Key, vec::KVec};

pub struct KGenMap<G: Key, K: Key, V, A: Alloc = GlobalAlloc> {
    next: Option<K>,
    vec: KVec<K, (G, KGenVal<K, V>), A>,
}


#[derive(Hash, Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyGen<G: Key, K: Key> {
    pub gen_key: G,
    pub key: K,
}

impl<G: Key, K: Key> KeyGen<G, K> {
    pub const ZERO : Self = Self::new(G::ZERO, K::ZERO);

    pub const fn new(gen_key: G, key: K) -> Self {
        Self { gen_key, key }
    }
}


enum KGenVal<K, V> {
    Occupied(V),
    Free { next: Option<K> },
}

impl<G: Key, K: Key, V> KGenMap<G, K, V, GlobalAlloc> {
    pub const fn new() -> Self {
        Self::new_in(GlobalAlloc)
    }
}


impl<G: Key, K: Key, V, A: Alloc> KGenMap<G, K, V, A> {
    pub const fn new_in(alloc: A) -> Self {
        Self {
            next: None,
            vec: KVec::new_in(alloc),
        }
    }


    pub fn iter(&self) -> impl Iterator<Item=(KeyGen<G, K>, &V)> {
        self.vec.iter()
            .enumerate()
            .filter_map(|(i, x)| match &x.1 {
                KGenVal::Occupied(v) => Some((KeyGen::new(x.0, unsafe { K::from_usize_unck(i) }), v)),
                KGenVal::Free { .. } => None,
            })
    }


    pub fn for_each_mut<F: FnMut(&mut V)>(&mut self, mut f: F) {
        for v in self.vec.iter_mut() {
            let KGenVal::Occupied(v) = &mut v.1
            else { continue };

            f(v);
        }
    }


    pub fn for_each<F: FnMut(&V)>(&self, mut f: F) {
        for v in self.vec.iter() {
            let KGenVal::Occupied(v) = &v.1
            else { continue };

            f(v);
        }
    }


    pub fn with_cap_in(alloc: A, cap: usize) -> Self {
        let mut this = Self::new_in(alloc);
        unsafe { this.vec.set_cap(cap) };
        this
    }


    pub fn insert(&mut self, value: V) -> KeyGen<G, K> {
        if let Some(next) = self.next {
            let (generation, slot) = &mut self.vec[next];
            *generation = unsafe { generation.add(1) };

            match slot {
                KGenVal::Free { next } => {
                    self.next = *next;
                },

                KGenVal::Occupied(_) => unreachable!(),
            }

            *slot = KGenVal::Occupied(value);

            return KeyGen::new(*generation, next)
        }

        let key = self.vec.push((G::ZERO, KGenVal::Occupied(value)));
        KeyGen::new(G::ZERO, key)
    }




    pub fn remove(&mut self, kg: KeyGen<G, K>) -> V {
        let (generation, slot) = &mut self.vec[kg.key];

        assert!(*generation == kg.gen_key,
                   "the generationeration of the slot does not match the key");

        assert!(!matches!(slot, KGenVal::Free { .. }),
                "the removed slot is already empty");

        let slot = core::mem::replace(slot, KGenVal::Free { next: self.next });
        self.next = Some(kg.key);

        match slot {
            KGenVal::Occupied(v) => v,
            KGenVal::Free { .. } => unreachable!(),
        }
    }


    pub fn get(&self, kg: KeyGen<G, K>) -> Option<&V> {
        let (generation, slot) = &self.vec[kg.key];
        
        if *generation != kg.gen_key { return None }

        match slot {
            KGenVal::Occupied(v) => Some(v),
            KGenVal::Free { .. } => None,
        }
    }


    pub fn get_mut(&mut self, kg: KeyGen<G, K>) -> Option<&mut V> {
        let (generation, slot) = &mut self.vec[kg.key];
        
        if *generation != kg.gen_key { return None }

        match slot {
            KGenVal::Occupied(v) => Some(v),
            KGenVal::Free { .. } => None,
        }
    }



    pub fn get_many_mut<'a>(&'a mut self, kg: KeyGen<G, K>, kg2: KeyGen<G, K>) -> [Option<&'a mut V>; 2] {
        assert!(kg != kg2);

        let [kg1v, kg2v] = self.vec.get_disjoint_mut([kg.key.usize(), kg2.key.usize()]).unwrap();
        
        [
        {
                let (generation, slot) = kg1v;
                if *generation != kg.gen_key { None }
                else {
                    match slot {
                        KGenVal::Occupied(v) => Some(v),
                        KGenVal::Free { .. } => None,
                    }
                }
        },
        {
                let (generation, slot) = kg2v;
                if *generation != kg2.gen_key { None }
                    else {
                    match slot {
                        KGenVal::Occupied(v) => Some(v),
                        KGenVal::Free { .. } => None,
                    }
                }
        },
        ]
        
    }

}


impl<G: Key, K: Key, V, A: Alloc> Index<KeyGen<G, K>> for KGenMap<G, K, V, A> {
    type Output = V;

    fn index(&self, kg: KeyGen<G, K>) -> &Self::Output {
        let (generation, slot) = &self.vec[kg.key];

        assert!(*generation == kg.gen_key,
                   "the generationeration of the slot does not match the key");

        match slot {
            KGenVal::Occupied(v) => v,
            KGenVal::Free { .. } => panic!("the accessed slot is empty"),
        }
    }
}


impl<G: Key, K: Key, V, A: Alloc> IndexMut<KeyGen<G, K>> for KGenMap<G, K, V, A> {
    fn index_mut(&mut self, kg: KeyGen<G, K>) -> &mut Self::Output {
        let (generation, slot) = &mut self.vec[kg.key];

        assert!(*generation == kg.gen_key,
                   "the generationeration of the slot does not match the key");

        match slot {
            KGenVal::Occupied(v) => v,
            KGenVal::Free { .. } => panic!("the accessed slot is empty"),
        }
    }
}
