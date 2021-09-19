extends Control


# Declare member variables here. Examples:
# var a = 2
# var b = "text"


# Called when the node enters the scene tree for the first time.
func _ready():
	$Background.material.set("shader_param/animation_progress", 0)


# Called every frame. 'delta' is the elapsed time since the previous frame.
#func _process(delta):
#	pass


func _on_Button_pressed():
	var _a = $Network.ws.connect_to_url("ws://" + $LineEdit.text + ":8123")
