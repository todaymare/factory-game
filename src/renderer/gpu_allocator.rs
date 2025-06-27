use std::marker::PhantomData;

use bytemuck::Pod;
use rand::seq::IndexedRandom;
use tracing::error;
use wgpu::{util::StagingBelt, Buffer, BufferUsages, CommandEncoder};

use crate::buddy_allocator::BuddyAllocator;

use super::ssbo::{ResizableBuffer, SSBO};

pub struct GPUAllocator<T> {
    pub allocator: BuddyAllocator,
    pub ssbo: ResizableBuffer<T>,
    buffer: Vec<T>,
    marker: PhantomData<T>,
}


#[derive(Debug, Clone, Copy)]
pub struct GpuPointer<T> {
    pub size: usize,
    pub offset: usize,
    marker: PhantomData<T>,
}


impl<T: Pod + core::fmt::Debug + PartialEq> GPUAllocator<T> {
    pub fn new(device: &wgpu::Device, initial: usize) -> Self {
        Self {
            allocator: BuddyAllocator::new(initial),
            ssbo: ResizableBuffer::new(device, BufferUsages::COPY_DST | BufferUsages::COPY_SRC | BufferUsages::VERTEX | BufferUsages::INDEX, initial),
            marker: PhantomData,
            buffer: vec![T::zeroed(); initial],
        }
    }


    pub fn allocate_slice(&mut self, belt: &mut StagingBelt, encoder: &mut CommandEncoder, device: &wgpu::Device, slice: &[T]) -> GpuPointer<T> {
        let Some(index) = self.allocator.alloc(slice.len())
        else {
            error!("trying to allocate a buffer with size '{}' in bytes", slice.len() * size_of::<T>());
            self.allocator.arrays.last_mut().unwrap().push(self.ssbo.len);
            self.allocator.arrays.push(vec![]);
            self.allocator.try_expand(self.allocator.arrays.len()-1);

            self.ssbo.resize(device, encoder, self.ssbo.len * 2);
            self.buffer.resize(self.ssbo.len * 2, T::zeroed());
            return self.allocate_slice(belt, encoder, device, slice);
        };

        self.ssbo.write(belt, encoder, device, index as u64, slice);
        for i in &self.buffer[index..index+slice.len()] {
            assert!(i == &T::zeroed());
        }

        GpuPointer { size: slice.len(), offset: index, marker: PhantomData }
    }


    pub fn free(&mut self, ptr: GpuPointer<T>) {
        self.allocator.free(ptr.offset, ptr.size);
        for i in &mut self.buffer[ptr.offset..ptr.offset+ptr.size] {
            *i = T::zeroed();
        }
    }
}


impl<T> GpuPointer<T> {
    pub fn offset_in_bytes(self) -> usize {
        self.offset * size_of::<T>()
    }

    pub fn size_in_bytes(self) -> usize {
        self.size * size_of::<T>()
    }
}
