extends Node

var ws := WebSocketClient.new()

func _init():
	var _a = ws.connect("data_received", self, "data_recv")

func data_recv():
	print(JSON.parse(ws.get_peer(1).get_packet().get_string_from_utf8()).result["event_type"])

func _process(_delta):
	ws.poll()
