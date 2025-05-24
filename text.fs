#version 330 core

in vec2 TexCoords;
out vec4 color;

uniform sampler2D text;
uniform vec3 textColor;

void main()
{
    vec2 uv = TexCoords.xy;

    float alpha = texture(text, uv).r;

    if (alpha < 0.4) {
        float pxOffset = 0.01;
        // Sample 8 surrounding texels
        float outline = 0.0;
        outline += texture(text, uv + vec2(-pxOffset,  0)).r;
        outline += texture(text, uv + vec2( pxOffset,  0)).r;
        outline += texture(text, uv + vec2( 0, -pxOffset)).r;
        outline += texture(text, uv + vec2( 0,  pxOffset)).r;

        // Corners (optional)
        outline += texture(text, uv + vec2(-pxOffset, -pxOffset)).r;
        outline += texture(text, uv + vec2(-pxOffset,  pxOffset)).r;
        outline += texture(text, uv + vec2( pxOffset, -pxOffset)).r;
        outline += texture(text, uv + vec2( pxOffset,  pxOffset)).r;

        if (outline > 0.0) {
            color = vec4(vec3(0), 1.0) * outline;
            return;
        } else {
            discard;
        }
    }

    // Normal glyph render
    vec4 sampled = vec4(1.0, 1.0, 1.0, alpha);
    color = vec4(textColor, 1.0) * sampled;
}
