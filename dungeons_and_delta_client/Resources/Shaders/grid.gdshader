shader_type canvas_item;

uniform vec2 square_size = vec2(60, 60);
uniform vec4 color_one = vec4(0.5, 0, 0.5, 1);
uniform float box_scale_multiplier = 1.5;
uniform vec4 color_two = vec4(0.5, 0, 0.5, 1);
uniform vec4 background = vec4(0, 0, 0, 1);
uniform bool grid_displayed = true;

void fragment(){
	
	COLOR = background;
	
	// Bottom squares
	if (grid_displayed) {
		if ((int(FRAGCOORD.x - 15.0) % int(square_size.x)) - (int(TIME * -10.0) % int(square_size.x)) == 0) {
			COLOR = vec4((color_two.x * 0.25), (color_two.y * 0.25), (color_two.z * 0.25), 1.0);
		}
		
		if ((int(FRAGCOORD.y + 15.0) % int(square_size.y)) - (int(TIME * 10.0) % int(square_size.y)) == 0) {
			COLOR = vec4((color_two.x * 0.25), (color_two.y * 0.25), (color_two.z * 0.25), 1.0);
		}
		
		// Top squares
		
		if ((int(FRAGCOORD.x) % int(square_size.x * box_scale_multiplier)) - (int(TIME * 10.0) % int(square_size.x * box_scale_multiplier)) == 0) {
			COLOR = vec4((color_one.x * 0.75), (color_one.y * 0.75), (color_one.z * 0.75), 1.0);
		}
		
		if ((int(FRAGCOORD.y) % int(square_size.y * box_scale_multiplier)) - (int(TIME * -10.0) % int(square_size.y * box_scale_multiplier)) == 0) {
			COLOR = vec4((color_one.x * 0.75), (color_one.y * 0.75), (color_one.z * 0.75), 1.0);
		}
	}
}