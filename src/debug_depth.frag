#version 450

// We do all light calculations in tangent space to avoid having to do matrix multiplications
// for every fragment (in order to convert normal sampled from normal map into world space).

layout(location=0) in vec3 v_position;       // tangent space
layout(location=1) in vec3 v_light_position; // tangent space
layout(location=2) in vec3 v_view_position;  // tangent space
layout(location=3) in vec2 v_tex_coords;

layout(location=0) out vec4 f_color;

layout(set = 0, binding = 0) uniform texture2D t_diffuse;
layout(set = 0, binding = 1) uniform sampler s_diffuse;
layout(set = 0, binding = 2) uniform texture2D t_normal;
layout(set = 0, binding = 3) uniform sampler s_normal;
layout(set = 3, binding = 0) uniform Light {
  vec3 light_position;
  vec3 light_color;
};

void main() {
  vec4 object_color = texture(sampler2D(t_diffuse, s_diffuse), v_tex_coords);
  f_color = object_color;
  /*
  float x;
  if (object_color.x == 1.0) {
    x = 1.0;
  } else {
    x = 0.0;
  }
  f_color = vec4(x, x, x, 1.0);
  */
}
