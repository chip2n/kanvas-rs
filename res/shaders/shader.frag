#version 450

// We do all light calculations in tangent space to avoid having to do matrix multiplications
// for every fragment (in order to convert normal sampled from normal map into world space).

// If you change this, make sure the location qualifiers are correct
const int MAX_LIGHTS = 2;

layout(location=0) in vec3 v_position;       // tangent space
layout(location=1) in vec3 v_light_positions[MAX_LIGHTS]; // tangent space
layout(location=3) in vec3 v_view_position;  // tangent space
layout(location=4) in vec2 v_tex_coords;
layout(location=5) in vec3 v_position_world_space;

layout(location=0) out vec4 f_color;

layout(set = 0, binding = 0) uniform texture2D t_diffuse;
layout(set = 0, binding = 1) uniform sampler s_diffuse;
layout(set = 0, binding = 2) uniform texture2D t_normal;
layout(set = 0, binding = 3) uniform sampler s_normal;

layout(set = 3, binding = 0) uniform Light {
  vec3 light_positions[MAX_LIGHTS];
  vec3 light_colors[MAX_LIGHTS];
};
layout(set = 3, binding = 1) uniform textureCube shadow_tex;
layout(set = 3, binding = 2) uniform sampler shadow_map;
layout(set = 3, binding = 3) uniform LightConfig {
  bool shadows_enabled;
};

//float z_near = 0.1;
float z_near = 0.1;
float z_far = 100;

vec3 sample_offset_directions[20] = vec3[](
   vec3( 1,  1,  1), vec3( 1, -1,  1), vec3(-1, -1,  1), vec3(-1,  1,  1),
   vec3( 1,  1, -1), vec3( 1, -1, -1), vec3(-1, -1, -1), vec3(-1,  1, -1),
   vec3( 1,  1,  0), vec3( 1, -1,  0), vec3(-1, -1,  0), vec3(-1,  1,  0),
   vec3( 1,  0,  1), vec3(-1,  0,  1), vec3( 1,  0, -1), vec3(-1,  0, -1),
   vec3( 0,  1,  1), vec3( 0, -1,  1), vec3( 0, -1, -1), vec3( 0,  1, -1)
);

float calculate_shadow(vec3 light_position) {
  if (!shadows_enabled) {
    return 0.0;
  }

  vec3 frag_to_light = v_position_world_space - light_position;
  frag_to_light *= vec3(1, 1, -1); // TODO Not sure why this is needed

  float current_depth = length(frag_to_light);

  // Do PCF for smoother shadows

  float shadow = 0.0;
  float bias = 0.15;
  int samples = 20;
  float view_distance = length(v_view_position - v_position_world_space);

  // By letting the disk radius depend on view distance, we get softer shadows when far away
  // and sharper shadows when close by
  float disk_radius = (1.0 + (view_distance / z_far)) / 25.0;

  for (int i = 0; i < samples; ++i) {
    float closest_depth = texture(samplerCube(shadow_tex, shadow_map), frag_to_light + sample_offset_directions[i] * disk_radius).r;
    closest_depth *= z_far; // undo linear [0,1] mapping done in shadow pass fragment stage
    if (current_depth - bias > closest_depth) {
      shadow += 1.0;
    }
  }
  shadow /= float(samples);

  return shadow;
}

vec3 calculate_light(vec3 light_position, vec3 light_position_tangent_space, vec3 light_color, bool shadows) {
  // Obtain normal from the normal map
  vec4 object_normal = texture(sampler2D(t_normal, s_normal), v_tex_coords);

  // Normals are stored in ranges [0..1], but we need them in [-1, 1]
  vec3 normal = normalize(object_normal.rgb * 2.0 - 1.0);
  
  vec3 light_dir = normalize(light_position_tangent_space - v_position);

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
  float shadow = 0.0;
  if (shadows) {
    shadow = calculate_shadow(light_position);
  }

  return (ambient_color + (1.0 - shadow) * (diffuse_color + specular_color)) * object_color.xyz;
}

void main() {
  vec3 result = vec3(0, 0, 0);

  for (int i = 0; i < MAX_LIGHTS; i++) {
    vec3 light_position = light_positions[i];
    vec3 light_position_tangent_space = v_light_positions[i];
    vec3 light_color = light_colors[i];

    result += calculate_light(light_position, light_position_tangent_space, light_color, i == 0);
  }
  
  f_color = vec4(result, 1.0);
}
