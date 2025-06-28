use sti::{key::Key, vec::KVec};

#[derive(Debug)]
pub struct FreeKVec<K: Key, V> {
    data: KVec<K, V>,
    free: Vec<K>,
}


impl<K: Key, V> FreeKVec<K, V> {
    pub fn new() -> Self {
        Self {
            data: KVec::new(),
            free: vec![],
        }
    }


    pub fn push(&mut self, value: V) -> K { 
        self.data.push(value)
    }


    pub fn remove(&mut self, key: K) {
        self.free.push(key);
    }


    pub fn get_mut(&mut self, key: K) -> &mut V {
        debug_assert!(!self.free.contains(&key), "tried to access already freed data");
        &mut self.data[key]
    }


    pub fn as_slice(&self) -> &[V] {
        &self.data
    }
}
