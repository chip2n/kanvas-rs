#version 450

const int MAX_LIGHTS = 2;

layout(location=0) in vec3 a_position;
layout(location=1) in vec2 a_tex_coords;
layout(location=2) in vec3 a_normal;
layout(location=3) in vec3 a_tangent;
layout(location=4) in vec3 a_bitangent;

layout(location=0) out vec3 v_position;       // tangent space
layout(location=1) out vec3 v_light_positions[MAX_LIGHTS]; // tangent space
layout(location=3) out vec3 v_view_position;  // tangent space
layout(location=4) out vec2 v_tex_coords;
layout(location=5) out vec3 v_position_world_space;

layout(set=1, binding=0) uniform Globals {
  vec3 u_view_position; // world space
  mat4 u_view_proj;
};

layout(set=2, binding=0) buffer Instances {
  mat4 s_models[];
};

layout(set=3, binding=0) uniform Light {
  vec3 light_positions[MAX_LIGHTS]; // world space
  vec3 light_colors[MAX_LIGHTS];
};

void main() {
  // Get the model matrix which will perform model->world transformation
  mat4 model_matrix = s_models[gl_InstanceIndex];

  // World position is a simple matrix multiplication of the model matrix and the model space position
  vec4 world_position = model_matrix * vec4(a_position, 1.0);

  // Calculate normal matrix which will perform model-> world transformation for normals.
  // This is essentially just the rotation data from the model matrix (no scale or translations since these
  // should not affect normals).
  // TODO This should be calculated on the CPU and passed in with the other uniforms since inversion is expensive
  mat3 normal_matrix = mat3(transpose(inverse(model_matrix)));

  // Calculate tangent matrix which will perform world->tangent transformation
  vec3 normal = normalize(normal_matrix * a_normal);
  vec3 tangent = normalize(normal_matrix * a_tangent);
  vec3 bitangent = normalize(normal_matrix * a_bitangent);
  mat3 tangent_matrix = transpose(mat3(tangent, bitangent, normal));

  vec3 light_positions_tangent_space[MAX_LIGHTS];
  for (int i = 0; i < MAX_LIGHTS; i++) {
    light_positions_tangent_space[i] = tangent_matrix * light_positions[i];
  }

  v_position = tangent_matrix * world_position.xyz;
  v_light_positions = light_positions_tangent_space;
  v_view_position = tangent_matrix * u_view_position;
  v_tex_coords = a_tex_coords;
  v_position_world_space = vec3(world_position);

  gl_Position = u_view_proj * world_position;
}
