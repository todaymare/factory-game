#version 330 core
layout (location = 0) in uint pos;
layout (location = 1) in uint colour;
//layout (location = 1) in vec3 aNormal;
//layout (location = 1) in vec4 aColour;

out vec4 Colour;

uniform mat4 model;
uniform mat4 view;
uniform mat4 projection;
uniform vec4 modulate;


vec3 unpack_voxel_pos() {
    uint x =  pos        & 63u;       // bits 0-5 (6 bits)
    uint y = (pos >> 6u) & 63u;       // bits 6-11 (6 bits)
    uint z = (pos >> 12u) & 63u;      // bits 12-17 (6 bits)
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
    vec3 pos = unpack_voxel_pos();
    vec4 colour = unpack_voxel_color();
    Colour = colour * modulate;
    gl_Position = projection * view * model * vec4(pos, 1.0);
}

