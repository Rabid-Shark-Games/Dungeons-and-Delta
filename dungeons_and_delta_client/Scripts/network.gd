extends Node

var ws := WebSocketClient.new()

func _init():
	var _a = ws.connect("data_received", self, "data_recv")
	var _b = ws.connect("connection_established", self, "connected")

func data_recv():
	print(JSON.parse(ws.get_peer(1).get_packet().get_string_from_utf8()).result["event_type"])

func connected(proto):
	get_node("/root/Combat/AnimationPlayer").play("fadein")
	get_node("/root/Combat/AudioStreamPlayer").playing = true

func _process(_delta):
	ws.poll()
