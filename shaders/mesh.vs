#version 330 core
layout (location = 0) in vec3 aPos;
//layout (location = 1) in vec3 aNormal;
layout (location = 1) in uint aColour;

out vec4 Colour;

uniform mat4 model;
uniform mat4 view;
uniform mat4 projection;
uniform vec4 modulate;


vec4 unpack_color(uint colour) {
    uint r =  (colour >> 24u) & 0xFFu;
    uint g =  (colour >> 16u) & 0xFFu;
    uint b =  (colour >> 8u)  & 0xFFu;
    uint a =  colour         & 0xFFu;
    return vec4(r, g, b, a) / 255.0;
}

void main()
{
    vec4 colour = unpack_color(aColour);
    Colour = colour * modulate;
    gl_Position = projection * view * model * vec4(aPos, 1.0);
}
