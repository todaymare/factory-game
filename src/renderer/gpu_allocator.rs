use std::marker::PhantomData;

use bytemuck::Pod;
use tracing::warn;
use wgpu::{util::StagingBelt, BufferUsages, CommandEncoder};

use crate::buddy_allocator::BuddyAllocator;

use super::ssbo::ResizableBuffer;

pub struct GPUAllocator<T> {
    pub allocator: BuddyAllocator,
    pub ssbo: ResizableBuffer<T>,
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
            ssbo: ResizableBuffer::new("gpu-allocator", device, BufferUsages::COPY_DST | BufferUsages::COPY_SRC | BufferUsages::VERTEX | BufferUsages::INDEX, initial),
            marker: PhantomData,
        }
    }


    pub fn allocate_slice(&mut self, belt: &mut StagingBelt, encoder: &mut CommandEncoder, device: &wgpu::Device, slice: &[T]) -> GpuPointer<T> {
        let Some(index) = self.allocator.alloc(slice.len())
        else {
            warn!("resizing while trying to allocate a buffer with size '{}' in bytes", slice.len() * size_of::<T>());
            self.allocator.arrays.last_mut().unwrap().push(self.ssbo.len);
            self.allocator.arrays.push(vec![]);
            self.allocator.try_expand(self.allocator.arrays.len()-1);

            self.ssbo.resize(device, encoder, self.ssbo.len * 2);
            return self.allocate_slice(belt, encoder, device, slice);
        };

        self.ssbo.write(belt, encoder, device, index, slice);
        GpuPointer { size: slice.len(), offset: index, marker: PhantomData }
    }


    pub fn free(&mut self, ptr: GpuPointer<T>) {
        self.allocator.free(ptr.offset, ptr.size);
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
