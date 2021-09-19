extends ColorRect

export var animation_progress := 1.0

func _process(delta):
	material.set("shader_param/animation_progress", animation_progress)
