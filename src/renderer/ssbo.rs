use std::{marker::PhantomData, num::NonZeroU64};

use bytemuck::Pod;
use tracing::{error, warn};
use wgpu::util::StagingBelt;

pub struct SSBO<T> {
    pub buffer: ResizableBuffer<T>,
    bind_group: wgpu::BindGroup,
    layout: wgpu::BindGroupLayout,
    marker: PhantomData<T>,
}


pub struct ResizableBuffer<T> {
    pub buffer: wgpu::Buffer,
    name: &'static str,
    pub len: usize,
    usage: wgpu::BufferUsages,
    marker: PhantomData<T>,
}


impl<T: Pod + std::fmt::Debug> ResizableBuffer<T> {
    pub fn new(name: &'static str, device: &wgpu::Device, usage: wgpu::BufferUsages, len: usize) -> Self {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(name),
            size: (len * size_of::<T>()) as u64,
            usage,
            mapped_at_creation: false,
        });


        Self {
            buffer,
            name,
            len,
            marker: PhantomData,
            usage,
        }
    }


    pub fn resize(&mut self, device: &wgpu::Device, encoder: &mut wgpu::CommandEncoder, new_cap: usize) {
        if new_cap < self.len { return };

        let max_cap = device.limits().max_buffer_size/size_of::<T>() as u64;
        if max_cap < new_cap as u64 {
            warn!("[{}] buffer is too large to fit in the gpu. max capacity is '{max_cap}' requested capacity is '{new_cap}'. \
                  trimming down to max capacity", self.name);
        }

        let new_cap = new_cap.min(max_cap as usize);

        let new_buff = Self::new(self.name, device, self.usage, new_cap);

        encoder.copy_buffer_to_buffer(
            &self.buffer, 0,
            &new_buff.buffer, 0,
            (self.len * size_of::<T>()) as u64
        );

        *self = new_buff;
    }


    pub fn write(&self, belt: &mut StagingBelt, encoder: &mut wgpu::CommandEncoder, device: &wgpu::Device, offset: usize, mut data: &[T]) {
        if data.len() > self.len {
            error!("buffer is too small ('{}') to fit the data ('{}'). trimming the data to fit the buffer.",
                  self.len, data.len());
            data = &data[..self.len];
        }


        let mut view = belt.write_buffer(
            encoder, 
            &self.buffer,
            (offset * size_of::<T>()) as u64,
            NonZeroU64::new((data.len() * size_of::<T>()) as u64).unwrap(),
            device
        );

        view.copy_from_slice(bytemuck::cast_slice(data));
    }
}


impl<T: Pod + std::fmt::Debug> SSBO<T> {
    pub fn new(name: &'static str, device: &wgpu::Device, usages: wgpu::BufferUsages, data_len: usize) -> Self {
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

        let buffer = ResizableBuffer::new(name, device, usages, data_len);

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ssbo-buffer"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.buffer.as_entire_binding(),
            }],
        });

        Self {
            buffer,
            bind_group,
            layout,
            marker: PhantomData,
        }
    }


    pub fn resize(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        new_cap: usize,
    ) {
        if new_cap <= self.buffer.len {
            return; // no need to resize
        }

        self.buffer.resize(device, encoder, new_cap);

        // Re-create bind group with new buffer
        let new_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ssbo-resized-bind-group"),
            layout: &self.layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self.buffer.buffer.as_entire_binding(),
            }],
        });

        self.bind_group = new_bind_group;
    }

    /// Replaces the contents with new data, resizing if needed.
    pub fn update(&self, belt: &mut StagingBelt, encoder: &mut wgpu::CommandEncoder, device: &wgpu::Device, data: &[T]) {
        self.write(belt, encoder, device, 0, data);
    }


    pub fn write(&self, belt: &mut StagingBelt, encoder: &mut wgpu::CommandEncoder, device: &wgpu::Device, offset: usize, data: &[T]) {
        self.buffer.write(belt, encoder, device, offset, data);
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    pub fn layout(&self) -> &wgpu::BindGroupLayout {
        &self.layout
    }

    pub fn len(&self) -> usize {
        self.buffer.len
    }
}
