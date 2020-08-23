#version 450

layout(location=0) in vec3 v_position;       // tangent space
layout(location=1) in vec3 v_light_position; // tangent space
layout(location=2) in vec3 v_view_position;  // tangent space
layout(location=3) in vec2 v_tex_coords;

layout(location=0) out vec4 f_color;

layout(set = 0, binding = 0) uniform texture2D t_diffuse;
layout(set = 0, binding = 1) uniform samplerShadow s_diffuse;
layout(set = 0, binding = 2) uniform texture2D t_normal;
layout(set = 0, binding = 3) uniform sampler s_normal;
layout(set = 3, binding = 0) uniform Light {
  vec3 light_position;
  vec3 light_color;
};

void main() {
  float near = 0.1;
  float far = 100.0;
  float depth = texture(sampler2DShadow(t_diffuse, s_diffuse), vec3(v_tex_coords, 1));
  float r = depth;
  //float r = (2.0 * near * far) / (far + near - depth * (far - near));

  f_color = vec4(vec3(r), 1);
}
