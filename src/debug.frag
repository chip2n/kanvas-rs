#version 450

layout(location=0) in vec2 v_tex_coords;

layout(location=0) out vec4 f_color;

layout(set = 0, binding = 0) uniform texture2D t_depth;
layout(set = 0, binding = 1) uniform sampler s_depth;

float z_near = 0.1;
float z_far = 100.0;

// When using perspective projection, non-linear depth values are stored
// We need to transform them to linear in order to display them
// NOTE This should not be done for orthographic projections
float linearize_depth(float depth) {
    float z = depth * 2.0 - 1.0; // Back to NDC
    return (2.0 * z_near * z_far) / (z_far + z_near - z * (z_far - z_near));
}

void main() {
  float depth = texture(sampler2D(t_depth, s_depth), v_tex_coords).r;
  f_color = vec4(vec3(linearize_depth(depth) / z_far), 1.0);
}
