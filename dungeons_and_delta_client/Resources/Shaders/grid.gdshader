shader_type canvas_item;

uniform vec2 square_size = vec2(60, 60);
uniform vec4 color_one = vec4(0.5, 0, 0.5, 1);
uniform float box_scale_multiplier = 2;
uniform vec4 color_two = vec4(0.5, 0, 0.5, 1);
uniform vec4 background = vec4(0, 0, 0, 1);
uniform bool grid_displayed = true;
uniform float square_bounds = 2;

bool within(float x, float mi, float ma) {
	return x < ma && x > mi;
}

void fragment(){
	
	COLOR = background;
	
	// Bottom squares
	if (grid_displayed) {
		if (within(mod((UV.x * 1024.0) - 15.0, square_size.x) - mod(TIME * -10.0, square_size.x), -square_bounds, square_bounds)) {
			COLOR = vec4((color_two.x * 0.25), (color_two.y * 0.25), (color_two.z * 0.25), 1.0);
		}
		
		if (within(mod((UV.y * 600.0) + 15.0, square_size.y) - mod(TIME * -10.0, square_size.y), -square_bounds, square_bounds)) {
			COLOR = vec4((color_two.x * 0.25), (color_two.y * 0.25), (color_two.z * 0.25), 1.0);
		}
		
		// Top squares
		
		if (within(mod(UV.x * 1024.0, square_size.x * box_scale_multiplier) - mod(TIME * 10.0, square_size.x * box_scale_multiplier), -square_bounds, square_bounds)) {
			COLOR = vec4((color_one.x * 0.75), (color_one.y * 0.75), (color_one.z * 0.75), 1.0);
		}
		
		if (within(mod(UV.y * 600.0, square_size.y * box_scale_multiplier) - mod(TIME * 10.0, square_size.y * box_scale_multiplier), -square_bounds, square_bounds)) {
			COLOR = vec4((color_one.x * 0.75), (color_one.y * 0.75), (color_one.z * 0.75), 1.0);
		}
	}
}