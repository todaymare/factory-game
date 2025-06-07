#version 330 core

in vec2 TexCoords;
in vec4 Modulate;
out vec4 color;

uniform sampler2D text;

void main()
{
    vec2 uv = TexCoords.xy;

    // Normal glyph render
    vec4 sampled = texture(text, uv);
    if (sampled.a < 0.01)
        discard;
    color = Modulate * sampled;
}
