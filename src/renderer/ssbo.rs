use std::{marker::PhantomData, num::{NonZero, NonZeroU64}};

use bytemuck::Pod;
use rand::seq::IndexedRandom;
use tracing::info;
use wgpu::util::{DeviceExt, StagingBelt};

pub struct SSBO<T> {
    pub buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    layout: wgpu::BindGroupLayout,
    len: usize,
    usages: wgpu::BufferUsages,
    marker: PhantomData<T>,
}


pub struct ResizableBuffer<T> {
    pub buffer: wgpu::Buffer,
    pub len: usize,
    usage: wgpu::BufferUsages,
    marker: PhantomData<T>,
}


impl<T: Pod + std::fmt::Debug> ResizableBuffer<T> {
    pub fn new(device: &wgpu::Device, usage: wgpu::BufferUsages, len: usize) -> Self {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("buffer"),
            size: (len * size_of::<T>()) as u64,
            usage,
            mapped_at_creation: false,
        });

        Self {
            buffer,
            len,
            marker: PhantomData,
            usage,
        }
    }


    pub fn resize(&mut self, device: &wgpu::Device, encoder: &mut wgpu::CommandEncoder, new_cap: usize) {
        if new_cap < self.len { return };

        let new_buff = Self::new(device, self.usage, new_cap);

        encoder.copy_buffer_to_buffer(
            &self.buffer, 0,
            &new_buff.buffer, 0,
            (self.len * size_of::<T>()) as u64
        );

        *self = new_buff;
    }


    pub fn write(&self, belt: &mut StagingBelt, encoder: &mut wgpu::CommandEncoder, device: &wgpu::Device, offset: u64, data: &[T]) {
        let mut view = belt.write_buffer(
            encoder, 
            &self.buffer,
            offset * size_of::<T>() as u64,
            NonZeroU64::new((data.len() * size_of::<T>()) as u64).unwrap(),
            device
        );

        view.copy_from_slice(bytemuck::cast_slice(data));
    }
}


impl<T: Pod + std::fmt::Debug> SSBO<T> {
    pub fn new(device: &wgpu::Device, usages: wgpu::BufferUsages, data: &[T]) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("GpuVec3Buffer Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let (buffer, bind_group) = Self::create_buffer_and_group(device, usages, &layout, data);

        Self {
            buffer,
            bind_group,
            layout,
            len: data.len(),
            marker: PhantomData,
            usages,
        }
    }

    fn create_buffer_and_group(
        device: &wgpu::Device,
        usages: wgpu::BufferUsages,
        layout: &wgpu::BindGroupLayout,
        data: &[T],
    ) -> (wgpu::Buffer, wgpu::BindGroup) {
        assert!(!data.is_empty());
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("SSBO"),
            contents: bytemuck::cast_slice(data),
            usage: usages,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("GpuVec3Buffer Bind Group"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        (buffer, bind_group)
    }

    pub fn resize_to_capacity(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        new_capacity: usize,
    ) {
        use std::mem::size_of;

        if new_capacity <= self.len {
            return; // no need to resize
        }

        let old_size_bytes = self.len * size_of::<T>();
        let new_size_bytes = new_capacity * size_of::<T>();
        dbg!(new_size_bytes);

        // Create new larger buffer
        let new_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SSBO (resized)"),
            size: new_size_bytes as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::INDEX
                | wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        });

        // Copy old contents into new buffer
        encoder.copy_buffer_to_buffer(
            &self.buffer,
            0,
            &new_buffer,
            0,
            old_size_bytes as u64,
        );

        // Re-create bind group with new buffer
        let new_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SSBO (resized) Bind Group"),
            layout: &self.layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: new_buffer.as_entire_binding(),
            }],
        });

        self.buffer = new_buffer;
        self.bind_group = new_bind_group;
        self.len = new_capacity;
    }

    /// Replaces the contents with new data, resizing if needed.
    pub fn update(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, data: &[T]) {
        if data.len() >= self.len {
            info!("creating a new buffer with size {} bytes", data.len() * size_of::<T>());
            // Recreate buffer + bind group
            let (new_buffer, new_bind_group) = Self::create_buffer_and_group(device, self.usages, &self.layout, data);
            self.buffer = new_buffer;
            self.bind_group = new_bind_group;
            self.len = data.len();
        }

        self.write(queue, 0, data);
    }


    pub fn write(&self, queue: &wgpu::Queue, offset: u64, data: &[T]) {
        queue.write_buffer(&self.buffer, offset * size_of::<T>() as u64, bytemuck::cast_slice(data));
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    pub fn layout(&self) -> &wgpu::BindGroupLayout {
        &self.layout
    }

    pub fn len(&self) -> usize {
        self.len
    }
}
