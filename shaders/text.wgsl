struct VertexIn {
    @location(0) position: vec3<f32>,
    @location(1) uv      : vec2<f32>,
    @location(2) modulate: vec4<f32>,
};


struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0)       uv      : vec2<f32>,
    @location(1)       modulate: vec4<f32>,
}


struct FragmentIn{
    @builtin(position) position: vec4<f32>,
    @location(0)       uv      : vec2<f32>,
    @location(1)       modulate: vec4<f32>,
}


struct Uniforms {
    projection: mat4x4<f32>,
    view      : vec4<f32>,
}


@group(0) @binding(0)
var<uniform> u : Uniforms;

@group(1) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(1) @binding(1)
var s_diffuse: sampler;



@vertex
fn vs_main(vertex: VertexIn) -> VertexOut {
    var output : VertexOut;

    output.position = u.projection * vec4(vertex.position, 1.0);

    output.uv = vertex.uv;
    output.modulate = vertex.modulate;
    return output;
}


@fragment
fn fs_main(in: FragmentIn) -> @location(0) vec4<f32> {
    //return u.view;
    let alpha = f32(textureSample(t_diffuse, s_diffuse, in.uv).r);
    if alpha < 0.1 { discard; }

    let sampled = vec4(1.0, 1.0, 1.0, alpha);
    return vec4(in.modulate * sampled);
}
