struct VertexIn {
    @location(0) position: vec3<f32>,
    @location(1) colour  : u32,
};


struct InstanceIn {
    @location(2) modulate: vec4<f32>,
    @location(3) model0  : vec4<f32>,
    @location(4) model1  : vec4<f32>,
    @location(5) model2  : vec4<f32>,
    @location(6) model3  : vec4<f32>,
};


struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0)       modulate: vec4<f32>,
};


struct FragmentIn {
    @location(0) colour : vec4<f32>,
};


struct Uniforms {
    view       : mat4x4<f32>,
    projection : mat4x4<f32>,
};


@group(0) @binding(0)
var<uniform> u : Uniforms;


fn unpack_color(col : u32) -> vec4<f32> {
    let r = f32((col >> 24u) & 0xFFu) / 255.0;
    let g = f32((col >> 16u) & 0xFFu) / 255.0;
    let b = f32((col >> 8u)  & 0xFFu) / 255.0;
    let a = f32(col & 0xFFu) / 255.0;
    return vec4f(r, g, b, a);
}


@vertex
fn vs_main(vertex: VertexIn, instance: InstanceIn) -> VertexOut {
    var out: VertexOut;
    let colour   = unpack_color(vertex.colour);
    let model = mat4x4(instance.model0, instance.model1, instance.model2, instance.model3);
    out.modulate = instance.modulate * colour;
    out.position = u.projection * u.view * model * vec4f(vertex.position, 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    return vec4(
        pow(in.modulate.x, 2.2),
        pow(in.modulate.y, 2.2),
        pow(in.modulate.z, 2.2),
        pow(in.modulate.w, 2.2),
    );
}

