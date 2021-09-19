shader_type canvas_item;

uniform vec2 square_size = vec2(60, 60);
uniform vec4 color_one = vec4(0.5, 0, 0.5, 1);
uniform float box_scale_multiplier = 2;
uniform vec4 color_two = vec4(0.5, 0, 0.5, 1);
uniform vec4 background = vec4(0, 0, 0, 1);
uniform bool grid_displayed = true;
uniform float square_bounds = 2;
uniform float animation_progress = 1;

bool within(float x, float mi, float ma) {
	return x < ma && x > mi;
}

void fragment(){
	
	COLOR = background;
	
	// Bottom squares
	if (grid_displayed) {
		if (UV.y >= (1.0 - animation_progress)) {
			float transparency = 0.25 * min(1.0, animation_progress + 0.4) * color_two.a;
			
			if (within(mod((UV.x * 1024.0) - 15.0, square_size.x) - mod(TIME * -10.0, square_size.x), -square_bounds, square_bounds)) {
				COLOR = vec4(vec3(color_two.x, color_two.y, color_two.z) * transparency, 1.0);
			}
			
			if (within(mod((UV.y * 600.0) + 15.0, square_size.y) - mod(TIME * -10.0, square_size.y), -square_bounds, square_bounds)) {
				COLOR = vec4(vec3(color_two.x, color_two.y, color_two.z) * transparency, 1.0);
			}
		}
		
		// Top squares
		
		if (UV.y <= animation_progress) {
			float transparency = 0.75 * min(1.0, animation_progress + 0.4) * color_one.a;
			
			if (within(mod(UV.x * 1024.0, square_size.x * box_scale_multiplier) - mod(TIME * 10.0, square_size.x * box_scale_multiplier), -square_bounds, square_bounds)) {
				COLOR = vec4(vec3(color_one.x, color_one.y, color_one.z) * transparency, 1.0);
			}
			
			if (within(mod(UV.y * 600.0, square_size.y * box_scale_multiplier) - mod(TIME * 10.0, square_size.y * box_scale_multiplier), -square_bounds, square_bounds)) {
				COLOR = vec4(vec3(color_one.x, color_one.y, color_one.z) * transparency, 1.0);
			}
		}
	}
}