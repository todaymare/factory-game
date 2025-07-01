use wgpu::{util::RenderEncoder, wgc::api::Metal, wgt::{self, DrawIndexedIndirectArgs}, Buffer, Device, IndexFormat, RenderPass};

pub fn multi_draw_indexed_indirect(
    device: &Device,
    render_pass: &mut RenderPass,
    vertex_buff: &Buffer,
    instance_buff: &Buffer,
    indirect: &[DrawIndexedIndirectArgs],
    indirect_buff: &Buffer,
    index_buf: &Buffer,
    index_type: IndexFormat,
) {

//    #[cfg(not(target_os="macos"))]
    {
        render_pass.set_vertex_buffer(0, vertex_buff.slice(..));
        render_pass.set_vertex_buffer(1, instance_buff.slice(..));
        render_pass.set_index_buffer(index_buf.slice(..), index_type);

        render_pass.multi_draw_indexed_indirect(indirect_buff, 0, indirect.len() as _);
    }

/*
    #[cfg(target_os="macos")]
    {
        let device = unsafe { device.as_hal::<Metal, _, _>(|device| {
            let device = device.unwrap();
            let device = device.raw_device().lock();
            let device = &*device;

            let desc = IndirectCommandBufferDescriptor::new();
            desc.set_max_vertex_buffer_bind_count(2);
            desc.set_max_fragment_buffer_bind_count(2);

            let icb = device.new_indirect_command_buffer_with_descriptor(
                &desc,
                indirect.len() as u64,
                MTLResourceOptions::empty(),
            );


            for (i, indirect) in indirect.iter().enumerate() {
                let item = icb.indirect_render_command_at_index(i as u64);

                vertex_buff.as_hal::<Metal, _, _>(|b| {
                    let b = b.unwrap();
                    let b : &(metal::Buffer, wgt::BufferAddress) = unsafe { core::mem::transmute(b) };
                    item.set_vertex_buffer(0, Some(buf), offset);
                });
            }

        }) };
    }*/
}

