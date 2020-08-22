#version 450

layout(location=0) in vec3 a_position;

layout(location=0) out vec4 v_color;

layout(set=1, binding=0) uniform Uniforms {
  vec3 u_view_position;
  mat4 u_view_proj;
};

layout(set=2, binding=0) buffer Instances {
  mat4 s_models[];
};

// TODO unnecessary?
layout(set=3, binding=0) uniform Light {
  vec3 light_position;
  vec3 light_color;
};

void main() {
  mat4 model_matrix = s_models[gl_InstanceIndex];
  vec4 model_space = model_matrix * vec4(a_position, 1.0);

  v_color = vec4(light_color, 1.0);
  gl_Position = u_view_proj * model_space;
}
