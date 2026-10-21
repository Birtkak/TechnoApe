using Godot;

namespace P1HeadlessLoop;

/// <summary>
/// P1: proves a C# script compiles via the CLI and runs headless inside Godot.
/// Exit 0 = pass, 1 = fail.
/// </summary>
public partial class HelloCs : Node
{
    public override void _Ready()
    {
        var noise = new FastNoiseLite { Seed = 42 };
        float a = noise.GetNoise2D(10f, 20f);
        float b = noise.GetNoise2D(10f, 20f);
        bool ok = Mathf.IsEqualApprox(a, b) && a != 0f;
        GD.Print($"P1 csharp: dotnet={System.Environment.Version} noise_sample={a:F6} deterministic={ok}");
        GetTree().Quit(ok ? 0 : 1);
    }
}
