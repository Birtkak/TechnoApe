$godot = "C:\Users\dev\Tools\Godot\4.7.2-mono\Godot_v4.7.2-stable_mono_win64\Godot_v4.7.2-stable_mono_win64_console.exe"
& $godot --headless --path $PSScriptRoot --script res://smoke.gd
exit $LASTEXITCODE
