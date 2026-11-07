extends GdUnitTestSuite

## Server discovery (ADR 0145): a probe to localhost gets the answer back and the server is listed.


func test_probe_finds_a_responder_on_localhost() -> void:
	var port := 7900 + randi() % 50
	var responder := PacketPeerUDP.new()
	assert_int(responder.bind(port + 1)).is_equal(OK)
	var f := ServerFinder.new()
	f.probe(port)
	var got := false
	for i in 40:
		OS.delay_msec(25)
		if responder.get_available_packet_count() > 0:
			got = true
			break
	assert_bool(got).is_true()
	assert_str(responder.get_packet().get_string_from_utf8()).is_equal(ServerFinder.PROBE)
	responder.set_dest_address(responder.get_packet_ip(), responder.get_packet_port())
	responder.put_packet(ServerFinder.answer({"name": "unit", "seed": 7, "game_mode": "creative", "players": 1, "port": port, "proto": Net.PROTO}))
	for i in 40:
		OS.delay_msec(25)
		f.poll()
		if not f.servers.is_empty():
			break
	assert_dict(f.servers).contains_keys(["127.0.0.1:%d" % port])
	assert_str(f.servers["127.0.0.1:%d" % port]["name"]).is_equal("unit")
	responder.close()


func test_garbage_answers_are_ignored() -> void:
	var f := ServerFinder.new()
	f.poll()
	assert_dict(f.servers).is_empty()
