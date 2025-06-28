struct VertexIn {
    @location(0) pos    : vec3<i32>,
};


struct InstanceIn {
    @location(1) pos    : u32,
    @location(2) colour : u32,
    @location(3) w      : u32,
    @location(4) h      : u32,
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


struct Uniforms {
    view       : mat4x4<f32>,
    projection : mat4x4<f32>,
    modulate   : vec4<f32>,
    camera_pos : vec3<f32>,
    pad_00     : f32,
    fog_color  : vec3<f32>,
    pad_01     : f32,
    fog_density: f32,
    fog_start  : f32,
    fog_end    : f32,
    pad_02     : f32,
};

@group(0) @binding(0)
var<uniform> u : Uniforms;

@group(1) @binding(0)
var<storage, read> positions: array<vec4<f32>>;


const NORMAL_LOOKUP : array<vec3<f32>, 6> = array<vec3<f32>, 6>(
    vec3<f32>( 1.0, 0.0, 0.0),
    vec3<f32>( 0.0, 1.0, 0.0),
    vec3<f32>( 0.0, 0.0, 1.0),
    vec3<f32>(-1.0, 0.0, 0.0),
    vec3<f32>( 0.0,-1.0, 0.0),
    vec3<f32>( 0.0, 0.0,-1.0)
);

fn unpack_voxel_pos(pos: u32) -> vec3<f32> {
    let x = f32( (pos >> 3u) & 63u );
    let y = f32( (pos >> 9u) & 63u );
    let z = f32( (pos >> 15u) & 63u );
    return vec3<f32>(x, y, z);
}

fn unpack_voxel_color(colour: u32) -> vec4<f32> {
    let r = f32( (colour >> 24u) & 0xFFu ) / 255.0;
    let g = f32( (colour >> 16u) & 0xFFu ) / 255.0;
    let b = f32( (colour >> 8u)  & 0xFFu ) / 255.0;
    let a = f32(  colour        & 0xFFu ) / 255.0;
    return vec4<f32>(r, g, b, a);
}


@vertex
fn vs_main(offset: VertexIn, input: InstanceIn, @builtin(instance_index) instance_index: u32) -> VertexOut {
    var output: VertexOut;

    let normal_index : u32 = input.pos & 7u;

    let normal = NORMAL_LOOKUP[normal_index % 6u];

    let pos_vec = unpack_voxel_pos(input.pos);

    var o = offset.pos;


    // for the up and down faces width and height are flipped for some reason
    if (normal_index == 1 || normal_index == 4) {
        if normal_index == 1 { o = o.zyx; }
        if o.x == 1 { o.x += i32(input.h-1); }
        if o.z == 1 { o.z += i32(input.w-1); }

    }
    else {
        switch normal_index {
            case 3: { o = o.zyx; } // X-
            case 5: { o = o.zyx; } // Z-
            default: {}
        }

        if o.x == 1 { o.x += i32(input.w-1); }
        if o.z == 1 { o.z += i32(input.h-1); }

        switch normal_index {
            case 0: { o = o.yxz; } // X+
            case 3: { o = o.yxz; } // X-
            case 5: { o = o.xzy; } // Z+
            case 2: { o = o.xzy; } // Z-
            default: {}
        }

    };


    let colour = unpack_voxel_color(input.colour);
    let model = positions[instance_index];
    let world_pos = pos_vec + model.xyz + vec3<f32>(o);

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
    return in.colour;
    //return vec4(model.xyz * 0.01, 1);
    //return vec4(mix(u.fog_color, in.colour.xyz, fog_factor), in.colour.w);
}
