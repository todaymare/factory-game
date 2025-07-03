#[derive(Clone, PartialEq)]
pub struct BuddyAllocator {
    pub arrays: Vec<Vec<usize>>,
}


impl BuddyAllocator {
    pub fn new(size: usize) -> Self {
        let mut arrays = vec![vec![]; size.ilog2() as usize + 1];
        arrays.last_mut().unwrap().push(0);
        Self {
            arrays,
        }
    }


    pub fn alloc(&mut self, size: usize) -> Option<usize> {
        if size == 0 { return Some(0); }

        let buf = size.next_power_of_two().ilog2();
        let index = self.pop_array(buf as usize)?;

        Some(index)
    }


    pub fn free(&mut self, index: usize, size: usize) {
        let buf = size.next_power_of_two().ilog2();
        let slot = self.arrays[buf as usize].binary_search(&index).unwrap_err();
        self.arrays[buf as usize].insert(slot, index);

        self.try_expand(buf as usize);
    }


    pub fn try_expand(&mut self, index: usize) {
        if index >= self.arrays.len() - 1 { return };

        let size = 2usize.pow(index as u32);
        let [array, next_array] = self.arrays.get_disjoint_mut([index, index+1]).unwrap();

        let mut try_expand = false;
        let mut i = 0;
        while array.len() > 1 && i < array.len()-1 {
            let n1 = array[i];
            let n2 = array[i+1];

            if n2 - n1 == size {
                let slot = next_array.binary_search(&n1).unwrap_err();
                next_array.insert(slot, n1);

                array.remove(i);
                array.remove(i);
                try_expand = true;
            } else {
                i += 1;
            }
        }


        if try_expand {
            self.try_expand(index+1);
        }
    }


    pub fn pop_array(&mut self, index: usize) -> Option<usize> {
        if index >= self.arrays.len() { 
            return None;
        };

        if self.arrays[index].is_empty() {
            let big_array = self.pop_array(index+1)?;

            let first_half = big_array;
            let second_half = big_array + 2usize.pow(index as u32);
            self.arrays[index].extend([second_half, first_half]);
        }

        Some(self.arrays[index].pop().unwrap())
    }
}


impl core::fmt::Debug for BuddyAllocator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("GPUAllocator");
        for i in 0..self.arrays.len() {
            let arr = &self.arrays[i];
            s.field(&format!("{} elements", 2u32.pow(i as u32)), arr);
        }
        s.finish()
        
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name() {
        let mut alloc = BuddyAllocator::new(1024 * 1024);
        let base_alloc = alloc.clone();

        let mut stack = vec![];
        for i in 1..2 {
            stack.push((alloc.alloc(i * 7).unwrap(), i*7));
        }


        let mut cap = 100;
        while let Some((pop, size)) = stack.pop() {
            if cap != 0 && rand::random::<bool>() {
                stack.push((pop, size));
                stack.push((alloc.alloc(size*5).unwrap(), size*5));
                cap -= 1;
            } else {
                alloc.free(pop, size);
            }
        }

        assert_eq!(alloc, base_alloc);
    }


    #[test]
    fn test_buddy_allocator_alloc_free() {
        let mut allocator = BuddyAllocator::new(1024 * 1024);

        let idx1 = allocator.alloc(128).expect("Failed to allocate 128 bytes");
        let idx2 = allocator.alloc(256).expect("Failed to allocate 256 bytes");

        assert_ne!(idx1, idx2);

        allocator.free(idx1, 128);
        allocator.free(idx2, 256);

        let idx3 = allocator.alloc(384).expect("Failed to allocate 384 bytes after free");

        assert!(idx3 <= idx1.min(idx2));

        allocator.free(idx3, 384);
    }
}


