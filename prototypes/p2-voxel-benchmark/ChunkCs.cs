using System;
using System.Collections.Generic;
using Godot;

namespace P2VoxelBenchmark;

/// <summary>
/// Same chunk generator + naive culled mesher as chunk_gd.gd, in C#.
/// </summary>
public static class ChunkCs
{
    public const int Size = 16;
    public const int Volume = Size * Size * Size;

    public const byte Air = 0, Stone = 1, Dirt = 2, Grass = 3;

    private static readonly Vector3[] FaceNormals =
    {
        new(1, 0, 0), new(-1, 0, 0), new(0, 1, 0), new(0, -1, 0), new(0, 0, 1), new(0, 0, -1),
    };

    private static readonly Vector3I[] FaceOffsets =
    {
        new(1, 0, 0), new(-1, 0, 0), new(0, 1, 0), new(0, -1, 0), new(0, 0, 1), new(0, 0, -1),
    };

    private static readonly Vector3[][] FaceCorners =
    {
        new Vector3[] { new(1, 0, 0), new(1, 1, 0), new(1, 1, 1), new(1, 0, 1) },
        new Vector3[] { new(0, 0, 0), new(0, 0, 1), new(0, 1, 1), new(0, 1, 0) },
        new Vector3[] { new(0, 1, 0), new(0, 1, 1), new(1, 1, 1), new(1, 1, 0) },
        new Vector3[] { new(0, 0, 0), new(1, 0, 0), new(1, 0, 1), new(0, 0, 1) },
        new Vector3[] { new(0, 0, 1), new(1, 0, 1), new(1, 1, 1), new(0, 1, 1) },
        new Vector3[] { new(0, 0, 0), new(0, 1, 0), new(1, 1, 0), new(1, 0, 0) },
    };

    private static readonly Color[] TypeColors =
    {
        Colors.Black, new(0.5f, 0.5f, 0.52f), new(0.45f, 0.3f, 0.15f), new(0.25f, 0.6f, 0.2f),
    };

    public static byte[] Generate(int cx, int cy, int cz, FastNoiseLite noise2d, FastNoiseLite noise3d)
    {
        var voxels = new byte[Volume];
        int baseY = cy * Size;
        for (int z = 0; z < Size; z++)
        {
            int wz = cz * Size + z;
            for (int x = 0; x < Size; x++)
            {
                int wx = cx * Size + x;
                int h = (int)(noise2d.GetNoise2D(wx, wz) * 24.0f + 32.0f);
                for (int y = 0; y < Size; y++)
                {
                    int wy = baseY + y;
                    byte v = Air;
                    if (wy < h - 4) v = Stone;
                    else if (wy < h - 1) v = Dirt;
                    else if (wy < h) v = Grass;
                    if (v != Air && noise3d.GetNoise3D(wx, wy, wz) > 0.55f) v = Air;
                    voxels[x + Size * (y + Size * z)] = v;
                }
            }
        }
        return voxels;
    }

    public readonly record struct MeshData(Vector3[] Verts, Vector3[] Normals, Color[] Colors, int[] Indices);

    public static MeshData Mesh(byte[] voxels)
    {
        var verts = new List<Vector3>(4096);
        var normals = new List<Vector3>(4096);
        var colors = new List<Color>(4096);
        var indices = new List<int>(6144);
        for (int z = 0; z < Size; z++)
        for (int y = 0; y < Size; y++)
        for (int x = 0; x < Size; x++)
        {
            byte v = voxels[x + Size * (y + Size * z)];
            if (v == Air) continue;
            Color col = TypeColors[v];
            var pos = new Vector3(x, y, z);
            for (int f = 0; f < 6; f++)
            {
                Vector3I o = FaceOffsets[f];
                int nx = x + o.X, ny = y + o.Y, nz = z + o.Z;
                if (nx >= 0 && nx < Size && ny >= 0 && ny < Size && nz >= 0 && nz < Size
                    && voxels[nx + Size * (ny + Size * nz)] != Air)
                    continue;
                int b = verts.Count;
                Vector3 n = FaceNormals[f];
                Vector3[] c = FaceCorners[f];
                for (int k = 0; k < 4; k++)
                {
                    verts.Add(pos + c[k]);
                    normals.Add(n);
                    colors.Add(col);
                }
                indices.Add(b); indices.Add(b + 2); indices.Add(b + 1);
                indices.Add(b); indices.Add(b + 3); indices.Add(b + 2);
            }
        }
        return new MeshData(verts.ToArray(), normals.ToArray(), colors.ToArray(), indices.ToArray());
    }

    public static ArrayMesh BuildArrayMesh(MeshData d)
    {
        var mesh = new ArrayMesh();
        if (d.Verts.Length == 0) return mesh;
        var arrays = new Godot.Collections.Array();
        arrays.Resize((int)Godot.Mesh.ArrayType.Max);
        arrays[(int)Godot.Mesh.ArrayType.Vertex] = d.Verts;
        arrays[(int)Godot.Mesh.ArrayType.Normal] = d.Normals;
        arrays[(int)Godot.Mesh.ArrayType.Color] = d.Colors;
        arrays[(int)Godot.Mesh.ArrayType.Index] = d.Indices;
        mesh.AddSurfaceFromArrays(Godot.Mesh.PrimitiveType.Triangles, arrays);
        return mesh;
    }
}
