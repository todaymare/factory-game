struct VertexIn {
    @location(0) pos    : vec3<i32>,
};


struct InstanceIn {
    @location(1) p1 : u32,
    @location(2) p2 : u32,
    @location(3) offset : u32,
};


struct VertexOut {
    @builtin(position) position  : vec4<f32>,
    @location(0)      colour    : vec4<f32>,
    @location(1)      normal    : vec3<f32>,
    @location(2)      frag_pos  : vec3<f32>,
    @location(3)      v_distance: f32,
};


struct FragmentIn {
    @location(0) colour    : vec4<f32>,
    @location(1) normal    : vec3<f32>,
    @location(2) frag_pos  : vec3<f32>,
    @location(3) v_distance: f32,
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


const NORMAL_LOOKUP : array<vec3<f32>, 6> = array<vec3<f32>, 6>(
    vec3<f32>( 1.0, 0.0, 0.0),
    vec3<f32>( 0.0, 1.0, 0.0),
    vec3<f32>( 0.0, 0.0, 1.0),
    vec3<f32>(-1.0, 0.0, 0.0),
    vec3<f32>( 0.0,-1.0, 0.0),
    vec3<f32>( 0.0, 0.0,-1.0)
);


@vertex
fn vs_main(offset: VertexIn, input: InstanceIn) -> VertexOut {
    var output: VertexOut;

    // unpacking
    let p1 = input.p1;
    let p2 = input.p2;

    let x      =  p1          & 0x3Fu;
    let y      = (p1 >>  6u)  & 0x3Fu;
    let z      = (p1 >> 12u)  & 0x3Fu;
    let width  = (p1 >> 18u)  & 0x1Fu;
    let height = (p1 >> 23u)  & 0x1Fu;

    let r      = ((p1 >> 28u) & 0xFu) | ((p2 & 0xFu) << 4u);
    let g      = (p2 >>  4u) & 0xFFu;
    let b      = (p2 >> 12u) & 0xFFu;

    let pos = vec3<f32>(f32(x), f32(y), f32(z));
    let colour = vec3<f32>(f32(r), f32(g), f32(b)) / 255.0;


    // other stuff

    let mesh_data = positions[input.offset];
    let normal_index : u32 = mesh_data.normal;
    let model = vec3<f32>(mesh_data.offset.xyz*32-u.camera_block) - u.camera_offset;

    let normal = NORMAL_LOOKUP[normal_index];

    var o = offset.pos;


    // for the up and down faces width and height are flipped for some reason
    if (normal_index == 1 || normal_index == 4) {
        if normal_index == 1 { o = o.zyx; }
        if o.x == 1 { o.x += i32(height); }
        if o.z == 1 { o.z += i32(width); }

    }
    else {
        switch normal_index {
            case 3: { o = o.zyx; } // X-
            case 5: { o = o.zyx; } // Z-
            default: {}
        }

        if o.x == 1 { o.x += i32(width); }
        if o.z == 1 { o.z += i32(height); }

        switch normal_index {
            case 0: { o = o.yxz; } // X+
            case 3: { o = o.yxz; } // X-
            case 5: { o = o.xzy; } // Z+
            case 2: { o = o.xzy; } // Z-
            default: {}
        }

    };


    let world_pos = pos + model.xyz + vec3<f32>(o);

    let light_dir = normalize(vec3<f32>(0.5, 1.0, 0.3));

    let light = min(max(dot(normal, light_dir), 0.0) + 0.2, 1.0);

    output.colour = vec4<f32>(colour.rgb * light, 1.0) * u.modulate;

    output.position = u.projection * u.view * vec4<f32>(world_pos, 1.0);
    output.v_distance = length(world_pos);
    output.frag_pos = world_pos;

    return output;
}

@fragment
fn fs_main(in: FragmentIn) -> @location(0) vec4<f32> {
    let fog_factor = clamp((u.fog_end - in.v_distance) / (u.fog_end - u.fog_start), 0.0, 1.0);
    //return in.colour;
    //return vec4(model.xyz * 0.01, 1);
    return vec4(mix(u.fog_color, in.colour.xyz, fog_factor), in.colour.w);
}
