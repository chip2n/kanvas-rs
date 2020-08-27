#version 450

layout(location=0) in vec2 v_tex_coords;

layout(location=0) out vec4 f_color;

layout(set = 0, binding = 0) uniform texture2D t_depth;
layout(set = 0, binding = 1) uniform sampler s_depth;

void main() {
  float near = 0.1;
  float far = 100.0;
  float depth = texture(sampler2D(t_depth, s_depth), v_tex_coords).r;
  f_color = vec4(vec3(depth), 1.0);
}
