#version 450

layout(location=0) in vec3 a_position;
layout(location=1) in vec2 a_tex_coords;

layout(location=0) out vec2 v_tex_coords;

layout(set=1, binding=0) uniform Globals {
  vec3 u_view_position; // world space
  mat4 u_view_proj;
};

layout(set=2, binding=0) buffer Instances {
  mat4 s_models[];
};

void main() {
  mat4 model_matrix = s_models[gl_InstanceIndex];
  vec4 world_position = model_matrix * vec4(a_position, 1.0);

  v_tex_coords = a_tex_coords;
  gl_Position = u_view_proj * world_position;
}
