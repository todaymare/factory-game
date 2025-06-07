#version 330 core

in vec2 TexCoords;
in vec4 Modulate;
out vec4 color;

uniform sampler2D text;

void main()
{
    vec2 uv = TexCoords.xy;

    float alpha = texture(text, uv).r;
    if (alpha < 0.1) { discard; }

    // Normal glyph render
    vec4 sampled = vec4(1.0, 1.0, 1.0, alpha);
    color = Modulate * sampled;
}
