#version 330 core
layout (location = 0) in vec3 pos;
layout (location = 1) in vec2 uv;
layout (location = 2) in vec4 modulate;
out vec2 TexCoords;
out vec4 Modulate;

uniform mat4 projection;
uniform mat4 model;

void main()
{
    gl_Position = projection * vec4(pos, 1.0);
    TexCoords = uv;
    Modulate = modulate;
}
