#version 450

layout(location=0) in vec3 a_position;

layout(location=0) out vec4 v_position;

layout(set=0, binding=0) uniform ShadowUniforms {
  mat4 u_light_proj;
  vec3 light_position; // world space
};

layout(set=1, binding=0) buffer Instances {
  mat4 s_models[];
};

void main() {
  mat4 model_matrix = s_models[gl_InstanceIndex];
  vec4 world_position = model_matrix * vec4(a_position, 1.0);

  v_position = world_position;
  gl_Position = u_light_proj * vec4(world_position.xyz, 1.0);
}
