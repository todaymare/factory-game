#version 330 core
out vec4 FragColor;
in vec4 Colour;
in float v_distance;

uniform vec3 fog_color;
uniform float fog_density;
uniform float fog_end;
uniform float fog_start;


vec3 apply_fog(vec3 color) {
    float fogFactor = clamp((fog_end - v_distance) / (fog_end - fog_start), 0.0, 1.0);
    return mix(fog_color, color, fogFactor);
}


void main()
{    
    FragColor = vec4(apply_fog(Colour.xyz), Colour.w);
}

