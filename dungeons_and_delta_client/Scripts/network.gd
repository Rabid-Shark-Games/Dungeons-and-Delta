extends Node

var ws := WebSocketClient.new()

enum PlayerType {
	Host,
	Client
}

enum ClientType {
	SelectWho,
	Idle,
	Battle,
	Grid,
}

var player_type = null
var client_type = null

func _init():
	var _a = ws.connect("data_received", self, "data_recv")
	var _b = ws.connect("connection_established", self, "connected")

func data_recv():
	var stri = ws.get_peer(1).get_packet().get_string_from_utf8()
	var json = JSON.parse(stri).result
	print(stri)
	print(json)
	if json["event_type"] == "player_status":
		if json["status"] == "client":
			player_type = PlayerType.Client
			client_type = ClientType.SelectWho
			
			for name in json["possible_players"]:
				var button = Button.new()
				button.text = name
				button.connect("pressed", self, "select_player", [name])
				get_parent().get_node("MarginContainer/VBoxContainer").add_child(button)
				get_parent().get_node("MarginContainer").visible = true
		elif json["status"] == "host":
			player_type = PlayerType.Host
	elif player_type == PlayerType.Client:
		if json["event_type"] == "player_selected":
			if json["successful"] == true:
				get_parent().get_node("AnimationPlayer").play("fadein")
				get_parent().get_node("AudioStreamPlayer").play()
			else:
				get_tree().quit()

func select_player(name):
	print(name)
	var x = JSON.print({
		"event_type": "select_player",
		"char": name
	})
	print(x)
	#ws.put_packet("adadsadasd".to_utf8())
	ws.get_peer(1).put_packet(x.to_utf8())
	
	get_parent().get_node("CPUParticles2D").emitting = false
	get_parent().get_node("MarginContainer").visible = false

func connected(proto):
	# get_node("/root/Combat/AnimationPlayer").play("fadein")
	get_parent().get_node("CPUParticles2D").emitting = true
	# get_node("/root/Combat/AudioStreamPlayer").playing = true

func _process(_delta):
	ws.poll()
