#version 450

layout(location=0) in vec4 frag_pos;

layout(set=0, binding=0) uniform ShadowUniforms {
  mat4 u_light_proj;
  vec3 light_position; // world space
};

float z_far = 100;

void main() {
  // get distance between fragment and light source
  float light_distance = length(frag_pos.xyz - light_position);

  // map to [0;1] range by dividing by far_plane
  light_distance = light_distance / z_far;

  // write this as modified depth
  gl_FragDepth = light_distance;
}
