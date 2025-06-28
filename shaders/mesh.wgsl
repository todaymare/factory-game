// shared structs
struct VertexIn {
    @location(0) a_pos    : vec3<f32>,
    @location(1) a_colour : u32,
};

struct V2F {
    @builtin(position) clip_position : vec4<f32>,
    @location(0)      colour         : vec4<f32>,
};

struct FragmentIn {
    @location(0) colour : vec4<f32>,
};

struct Uniforms {
    model      : mat4x4<f32>,
    view       : mat4x4<f32>,
    projection : mat4x4<f32>,
    modulate   : vec4<f32>,
};
@group(0) @binding(0)
var<uniform> U : Uniforms;

fn unpack_color(col : u32) -> vec4<f32> {
    let r = f32((col >> 24u) & 0xFFu) / 255.0;
    let g = f32((col >> 16u) & 0xFFu) / 255.0;
    let b = f32((col >> 8u)  & 0xFFu) / 255.0;
    let a = f32(col & 0xFFu) / 255.0;
    return vec4f(r, g, b, a);
}

@vertex
fn vs_main(in: VertexIn) -> V2F {
    var out: V2F;
    let colour        = unpack_color(in.a_colour);
    out.colour        = colour * U.modulate;
    out.clip_position = U.projection * U.view * U.model * vec4f(in.a_pos, 1.0);
    return out;
}

@fragment
fn fs_main(in: FragmentIn) -> @location(0) vec4<f32> {
    return in.colour;
}

