#version 330 core
layout (location = 0) in uint pos;
layout (location = 1) in uint colour;

out vec4 Colour;
out vec3 Normal;
out vec3 FragPos;
out float v_distance;

uniform mat4 model;
uniform mat4 view;
uniform mat4 projection;
uniform vec4 modulate;
uniform vec3 cameraPos;


const vec3 NORMAL_LOOKUP[6] = vec3[](
    vec3(1, 0, 0),
    vec3(0, 1, 0),
    vec3(0, 0, 1),
    vec3(-1, 0, 0),
    vec3(0, -1, 0),
    vec3(0, 0, -1)
);


vec3 unpack_voxel_pos() {
    uint x = (pos >> 3u) & 63u;       // bits 0-5 (6 bits)
    uint y = (pos >> 9u) & 63u;       // bits 6-11 (6 bits)
    uint z = (pos >> 15u) & 63u;      // bits 12-17 (6 bits)
    return vec3(x, y, z);
}


vec4 unpack_voxel_color() {
    uint r =  (colour >> 24u) & 0xFFu;
    uint g =  (colour >> 16u) & 0xFFu;
    uint b =  (colour >> 8u)  & 0xFFu;
    uint a =  colour         & 0xFFu;
    return vec4(r, g, b, a) / 255.0;
}


void main()
{

    uint normal_index = pos & 7u;
    vec3 normal = NORMAL_LOOKUP[normal_index];
    vec3 pos = unpack_voxel_pos();
    vec4 colour = unpack_voxel_color();

    vec3 world_pos = (model * vec4(pos, 1.0)).xyz;
    vec3 light_dir = normalize(vec3(0.5, 1.0, 0.3)); // coming from top-front-right

    float light = min(max(dot(normal, light_dir), 0.0) + 0.2, 1);
    Colour = colour * modulate;
    Colour.a = 1;

    gl_Position = projection * view * model * vec4(pos, 1.0);
    v_distance = length(world_pos);

    FragPos = vec3(model * vec4(world_pos, 1.0));
    Normal = mat3(transpose(inverse(model))) * normal;  
}

