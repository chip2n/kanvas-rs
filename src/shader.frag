#version 450

// We do all light calculations in tangent space to avoid having to do matrix multiplications
// for every fragment (in order to convert normal sampled from normal map into world space).

layout(location=0) in vec3 v_position;       // tangent space
layout(location=1) in vec3 v_light_position; // tangent space
layout(location=2) in vec3 v_view_position;  // tangent space
layout(location=3) in vec2 v_tex_coords;
layout(location=4) in vec4 v_position_light_space;

layout(location=0) out vec4 f_color;

layout(set = 0, binding = 0) uniform texture2D t_diffuse;
layout(set = 0, binding = 1) uniform sampler s_diffuse;
layout(set = 0, binding = 2) uniform texture2D t_normal;
layout(set = 0, binding = 3) uniform sampler s_normal;
layout(set = 3, binding = 0) uniform Light {
  vec3 light_position;
  vec3 light_color;
};
layout(set = 3, binding = 1) uniform texture2D shadow_tex;
layout(set = 3, binding = 2) uniform sampler shadow_map;

const float z_near = 0.1;
const float z_far = 100;

float to_linear_depth(float depth) {
  float z_n = 2.0 * depth - 1.0;
  float z_e = 2.0 * z_near * z_far / (z_far + z_near - z_n * (z_far - z_near));
  return z_e;
}

// https://stackoverflow.com/questions/51108596/linearize-depth
float linearize_depth(float d) {
  return z_near * z_far / (z_far + d * (z_near - z_far));
}

float calculate_shadow() {
  // perform perspective divide
  vec3 proj_coords = v_position_light_space.xyz / v_position_light_space.w;

  // x and y is [-1, 1] in light space - we need it in [0,1] to be valid tex coords
  // positive y is up in light space, but down in tex coords, so flip it
  // z is [0, 1], so leave it unchanged
  proj_coords = vec3(proj_coords.x * 0.5 + 0.5, -proj_coords.y * 0.5 + 0.5, proj_coords.z);

  // if we're looking outside the shadow map, don't do any shadowing
  ivec2 size = textureSize(sampler2D(shadow_tex, shadow_map), 0);
  if (proj_coords.y > size.y || proj_coords.y < 0) {
    return 0.0;
  }
  if (proj_coords.x > size.x || proj_coords.x < 0) {
    return 0.0;
  }
  
  // get closest depth value from light's perspective (using [0,1] range as coords)
  float closest_depth = texture(sampler2D(shadow_tex, shadow_map), proj_coords.xy).r;

  // get depth of current fragment from light's perspective
  float current_depth = proj_coords.z;

  // prevent shadow acne with a bias
  //float bias = 0.005;
  float bias = 0.0;

  // check whether current frag pos is in shadow
  float shadow = current_depth - bias > closest_depth ? 1.0 : 0.0;
  //float shadow = closest_depth > 100.0 ? 1.0 : 0.0;
  //float shadow = to_linear_depth(closest_depth) > 100.0 ? 1.0 : 0.0;

  return shadow;
}

void main() {
  // Obtain normal from the normal map
  vec4 object_normal = texture(sampler2D(t_normal, s_normal), v_tex_coords);

  // Normals are stored in ranges [0..1], but we need them in [-1, 1]
  vec3 normal = normalize(object_normal.rgb * 2.0 - 1.0);
  
  vec3 light_dir = normalize(v_light_position - v_position);

  float diffuse_strength = max(dot(normal, light_dir), 0.0);
  vec3 diffuse_color = light_color * diffuse_strength;

  vec3 view_dir = normalize(v_view_position - v_position);
  vec3 half_dir = normalize(view_dir + light_dir);

  float shininess = 32;
  float specular = pow(max(dot(normal, half_dir), 0.0), shininess);
  float specular_strength = 0.5; // TODO sample from specular map
  vec3 specular_color = specular_strength * specular * light_color;

  vec4 object_color = texture(sampler2D(t_diffuse, s_diffuse), v_tex_coords);
  float ambient_strength = 0.1;
  vec3 ambient_color = light_color * ambient_strength;

  // calculate shadow
  float shadow = calculate_shadow();
  //float shadow = 0.0;

  vec3 result = (ambient_color + (1.0 - shadow) * (diffuse_color + specular_color)) * object_color.xyz;
  
  f_color = vec4(result, object_color.a);
}
