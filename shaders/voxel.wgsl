struct VertexIn {
    @location(0) pos    : vec3<i32>,
    @location(1) index  : u32,
};


struct InstanceIn {
    @location(2) p1     : u32,
    @location(3) id     : u32,
    @location(4) offset : u32,
};


struct VertexOut {
    @builtin(position) position  : vec4<f32>,
    @location(1)       normal    : vec3<f32>,
    @location(2)       frag_pos  : vec3<f32>,
    @location(3)       v_distance: f32,
    @location(4)       tex_coords: vec2<f32>,
    @location(5)       id        : u32,
    @location(6)       colour    : vec3<f32>,
};


struct FragmentIn {
    @location(1) normal    : vec3<f32>,
    @location(2) frag_pos  : vec3<f32>,
    @location(3) v_distance: f32,
    @location(4) tex_coords: vec2<f32>,
    @location(5)      id        : u32,
    @location(6) colour    : vec3<f32>,
};


struct ChunkMeshFramedata {
    offset: vec3<i32>,
    normal: u32,
}


struct Uniforms {
    view       : mat4x4<f32>,
    projection : mat4x4<f32>,
    modulate   : vec4<f32>,
    camera_block: vec3<i32>,
    pad_00     : f32,
    camera_offset: vec3<f32>,
    pad_01     : f32,
    fog_color  : vec3<f32>,
    pad_02     : f32,
    fog_density: f32,
    fog_start  : f32,
    fog_end    : f32,
    pad_03     : f32,
};

@group(0) @binding(0)
var<uniform> u : Uniforms;

@group(1) @binding(0)
var<storage, read> positions: array<ChunkMeshFramedata>;


@group(2) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(2) @binding(1)
var s_diffuse: sampler;

const NORMAL_LOOKUP : array<vec3<f32>, 6> = array<vec3<f32>, 6>(
    vec3<f32>( 1.0, 0.0, 0.0),
    vec3<f32>( 0.0, 1.0, 0.0),
    vec3<f32>( 0.0, 0.0, 1.0),
    vec3<f32>(-1.0, 0.0, 0.0),
    vec3<f32>( 0.0,-1.0, 0.0),
    vec3<f32>( 0.0, 0.0,-1.0)
);




@vertex
fn vs_main(vertex: VertexIn, input: InstanceIn) -> VertexOut {
    var output: VertexOut;

    // unpacking
    let p1 = input.p1;

    let x      =  p1          & 0x3Fu;
    let y      = (p1 >>  6u)  & 0x3Fu;
    let z      = (p1 >> 12u)  & 0x3Fu;
    let width  = (p1 >> 18u)  & 0x1Fu;
    let height = (p1 >> 23u)  & 0x1Fu;

    let pos = vec3<f32>(f32(x), f32(y), f32(z));

    // other stuff

    let mesh_data = positions[input.offset];
    let normal_index : u32 = mesh_data.normal;
    let model = vec3<f32>(mesh_data.offset.xyz*32-u.camera_block) - u.camera_offset;

    var normal = vec3<f32>(0.0);

    switch normal_index {
        case 0u: { normal = NORMAL_LOOKUP[0]; }
        case 1u: { normal = NORMAL_LOOKUP[1]; }
        case 2u: { normal = NORMAL_LOOKUP[2]; }
        case 3u: { normal = NORMAL_LOOKUP[3]; }
        case 4u: { normal = NORMAL_LOOKUP[4]; }
        case 5u: { normal = NORMAL_LOOKUP[5]; }
        default: {}
    }

    var o = vertex.pos;

    let ao = input.id >> 8u;
    let swap_diag = ((ao >> 8u) & 0x1u) == 1u;

    if swap_diag {
        if vertex.index == 2u { o.x = 0; o.z = 1; }
        if vertex.index == 5u { o.x = 1; o.z = 0; }
    }

    var ao_state = 3u;
    if o.x == 0 && o.z == 0 { ao_state = ((ao >> 0u) & 0x3u); }
    if o.x == 0 && o.z == 1 { ao_state = ((ao >> 2u) & 0x3u); }
    if o.x == 1 && o.z == 0 { ao_state = ((ao >> 4u) & 0x3u); }
    if o.x == 1 && o.z == 1 { ao_state = ((ao >> 6u) & 0x3u); }
    

    // for the up and down faces width and height are flipped for some reason
    if (normal_index == 1u || normal_index == 4u) {
        if normal_index == 1u { o = o.zyx; }

        if o.x == 1 { o.x += i32(height); }
        if o.z == 1 { o.z += i32(width); }

        output.tex_coords = vec2<f32>(f32(o.x), f32(o.z));
    }

    else {
        switch normal_index {
            case 3u: { o = o.zyx; } // X-
            case 5u: { o = o.zyx; } // Z-
            default: {}
        }

        if o.x == 1 { o.x += i32(width); }
        if o.z == 1 { o.z += i32(height); }

        let uv = vec2<f32>(f32(o.x), f32(o.z));

        switch normal_index {
            case 0u: { o = o.yxz; output.tex_coords = uv.yx; } // X+
            case 3u: { o = o.yxz; output.tex_coords = uv.yx; } // X-
            case 5u: { o = o.xzy; output.tex_coords = uv.xy; } // Z+
            case 2u: { o = o.xzy; output.tex_coords = uv.xy; } // Z-
            default: {}
        }

    };

    let ao_colours = vec4<f32>(0.0,0.05,0.1,1.0);
    output.colour = vec3(ao_colours[ao_state]);

    let world_pos = pos + model.xyz + vec3<f32>(o);

    let light_dir = normalize(vec3<f32>(0.5, 1.0, 0.3));

    let light = min(max(dot(normal, light_dir), 0.0) + 0.2, 1.0);

    output.position = u.projection * u.view * vec4<f32>(world_pos, 1.0);
    output.v_distance = length(world_pos);
    output.frag_pos = world_pos;
    output.id = input.id & 0xFFu;

    return output;
}


const TILE_SIZE: f32 = 1.0 / 256.0;
const PIXEL_SIZE : f32 = TILE_SIZE / 32.0;


fn lerp(a: f32, b: f32, t: f32) -> f32 {
    return a + t * (b - a);
}


@fragment
fn fs_main(in: FragmentIn) -> @location(0) vec4<f32> {
    let fog_factor = clamp((u.fog_end - in.v_distance) / (u.fog_end - u.fog_start), 0.0, 1.0);

    let base = f32(in.id) * TILE_SIZE;
    let max = base + TILE_SIZE;

    var v = clamp(lerp(base, max, in.tex_coords.x % 1.0), base, max);
    var colour = textureSample(t_diffuse, s_diffuse, vec2<f32>(v, in.tex_coords.y % 1.0));


    return vec4(mix(u.fog_color, colour.xyz * in.colour, fog_factor), colour.w);
}
