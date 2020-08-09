// shader.vert
#version 450

layout(location=0) in vec3 a_position;
layout(location=1) in vec2 a_tex_coords;
layout(location=2) in vec3 a_normal;

layout(location=0) out vec2 v_tex_coords;
layout(location=1) out vec3 v_normal;
layout(location=2) out vec3 v_position;

layout(set=1, binding=0)
uniform Uniforms {
  vec3 u_view_position;
  mat4 u_view_proj;
};

layout(set=1, binding=1)
buffer Instances {
  mat4 s_models[];
};

void main() {
  mat4 model_matrix = s_models[gl_InstanceIndex];
  // this can be calculated on the CPU and passed in with the other uniforms
  mat3 normal_matrix = mat3(transpose(inverse(model_matrix)));
  v_normal = normal_matrix * a_normal;

  v_tex_coords = a_tex_coords;
  vec4 model_space = s_models[gl_InstanceIndex] * vec4(a_position, 1.0);
  v_position = model_space.xyz;
  gl_Position = u_view_proj * model_space;
}
