#version 450

layout(location=0) in vec3 a_position;

layout(set=0, binding=0) uniform Uniforms {
  vec3 u_view_position;
  mat4 u_view_proj;
};

layout(set=1, binding=0) buffer Instances {
  mat4 s_models[];
};

void main() {
  mat4 model_matrix = s_models[gl_InstanceIndex];
  gl_Position = u_view_proj * model_matrix * vec4(a_position, 1.0);
}
