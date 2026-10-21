using System.Diagnostics;
using System.Text.Json;
using Godot;

namespace P2VoxelBenchmark;

/// <summary>
/// P2 part A, C#: same workload as bench_gd.gd. Run via bench_cs.tscn:
///   godot_console.exe --headless --path . res://bench_cs.tscn
/// </summary>
public partial class BenchCs : Node
{
    private const int ChunksPerAxis = 8;
    private const int ChunksY = 4;

    public override void _Ready()
    {
        var noise2d = new FastNoiseLite { Seed = 1337, Frequency = 0.01f };
        var noise3d = new FastNoiseLite { Seed = 7331, Frequency = 0.05f };

        var warm = ChunkCs.Generate(0, 1, 0, noise2d, noise3d);
        ChunkCs.BuildArrayMesh(ChunkCs.Mesh(warm));

        long genTicks = 0, meshTicks = 0, uploadTicks = 0;
        long faces = 0;
        int count = 0;
        var sw = new Stopwatch();
        for (int cz = 0; cz < ChunksPerAxis; cz++)
        for (int cy = 0; cy < ChunksY; cy++)
        for (int cx = 0; cx < ChunksPerAxis; cx++)
        {
            sw.Restart();
            var vox = ChunkCs.Generate(cx, cy, cz, noise2d, noise3d);
            long t1 = sw.ElapsedTicks;
            var md = ChunkCs.Mesh(vox);
            long t2 = sw.ElapsedTicks;
            ChunkCs.BuildArrayMesh(md);
            long t3 = sw.ElapsedTicks;
            genTicks += t1;
            meshTicks += t2 - t1;
            uploadTicks += t3 - t2;
            faces += md.Indices.Length / 6;
            count++;
        }

        double toMs = 1000.0 / Stopwatch.Frequency;
        double totalMs = (genTicks + meshTicks + uploadTicks) * toMs;
        var result = new
        {
            lang = "csharp",
            chunks = count,
            chunk_size = ChunkCs.Size,
            faces,
            gen_ms_per_chunk = System.Math.Round(genTicks * toMs / count, 3),
            mesh_ms_per_chunk = System.Math.Round(meshTicks * toMs / count, 3),
            upload_ms_per_chunk = System.Math.Round(uploadTicks * toMs / count, 3),
            total_ms = System.Math.Round(totalMs, 1),
            chunks_per_sec = System.Math.Round(count / (totalMs / 1000.0), 1),
        };
        GD.Print(JsonSerializer.Serialize(result));
        GetTree().Quit(0);
    }
}
