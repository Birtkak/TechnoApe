extends GdUnitTestSuite

## Client self-update (ADR 0166): the server's manifest lists the client folder's files with their hashes
## (not version.txt, not *.old) and a server may only name plain files beside the exe.


func test_safe_name_allows_plain_files_only() -> void:
	assert_bool(ClientUpdate.safe_name("TechnoApe.pck")).is_true()
	assert_bool(ClientUpdate.safe_name("libvoxel.windows.template_debug.x86_64.dll")).is_true()
	for bad in ["", "../evil.exe", "sub/x.pck", "C:\\x.exe", ".hidden", "update"]:
		assert_bool(ClientUpdate.safe_name(bad)).override_failure_message(bad).is_false()


func test_host_manifest_hashes_the_client_folder() -> void:
	var dir := "user://test_client_update"
	DirAccess.make_dir_recursive_absolute(dir)
	FileAccess.open(dir.path_join("version.txt"), FileAccess.WRITE).store_string("v7\n")
	var f := FileAccess.open(dir.path_join("Game.pck"), FileAccess.WRITE)
	f.store_string("pack bytes")
	f.close()
	FileAccess.open(dir.path_join("Game.pck.old"), FileAccess.WRITE).store_string("old")
	var host := ClientUpdate.Host.new(dir, 0, "dev")
	assert_str(host.version).is_equal("v7")
	var files: Array = host.manifest["files"]
	assert_int(files.size()).is_equal(1)
	assert_str(files[0]["name"]).is_equal("Game.pck")
	assert_int(files[0]["size"]).is_equal(10)
	assert_str(files[0]["sha256"]).is_equal("pack bytes".sha256_text())
	for n in DirAccess.get_files_at(dir):
		DirAccess.remove_absolute(dir.path_join(n))
