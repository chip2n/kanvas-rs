#version 450

layout(location=0) in vec2 v_tex_coords;

layout(location=0) out vec4 f_color;

layout(set = 0, binding = 0) uniform texture2D t_diffuse;
layout(set = 0, binding = 1) uniform sampler s_diffuse;

void main() {
  vec4 object_color = texture(sampler2D(t_diffuse, s_diffuse), v_tex_coords);
  // When the sampled color is transparent, we need to discard the entire fragment.
  // If we don't do this, it will be stored in the depth buffer and prevent anything
  // from being rendered behind it.
  // See https://www.khronos.org/opengl/wiki/Transparency_Sorting
  if (object_color.a < 0.5)
    discard;
  f_color = object_color;
}
