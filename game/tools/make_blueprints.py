"""Writes the New Stone Age blueprints (ADR 0059) to game/data/prefabs/blueprints/*.json in the
prefab format of ADR 0038 (PrefabStore: size, palette of material names, runs over the grid x fastest,
then z, then y). StoneAge copies them into user://prefabs/blueprints/ so the build menu (V, Prefabs)
lists them.

Run: python game/tools/make_blueprints.py
"""
import json
import os

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "data", "prefabs", "blueprints")
V = 8   # voxels per metre (12.5 cm voxels, ADR 0033)


def longhouse(length_m, width_m, wall_m, ridge_m):
    """A Linear Pottery longhouse: wattle-and-daub walls on oak posts, a door in one gable end, a
    thatched gable roof running the length, its eaves past the walls."""
    L, W = int(length_m * V), int(width_m * V)
    wall_h, ridge_h = int(wall_m * V), int(ridge_m * V)
    over = 3                       # eaves past the walls
    X, Z, Y = W + 2 * over, L + 2 * over, ridge_h + 2
    grid = {}
    t = 2                          # wall thickness (25 cm)
    for z in range(over, over + L):
        for x in range(over, over + W):
            edge_x = x < over + t or x >= over + W - t
            edge_z = z < over + t or z >= over + L - t
            if not (edge_x or edge_z):
                continue
            for y in range(wall_h):
                door = z < over + t and abs(x - (over + W // 2)) <= 4 and y < int(2.0 * V)
                if door:
                    continue
                post = (x - over) % (2 * V) < t and (z - over) % (2 * V) < t
                grid[(x, y, z)] = "wood" if post or (edge_x and edge_z) else "daub_wall"
    # gable ends up to the ridge, then the roof: two thatch slopes 3 voxels thick
    half = X / 2.0
    for z in range(Z):
        for x in range(X):
            d = abs(x + 0.5 - half)
            roof_y = wall_h + int((half - d) * (ridge_h - wall_h) / half)
            for k in range(3):
                y = roof_y - k
                if 0 <= y < Y:
                    grid[(x, y, z)] = "thatch"
            if over <= z < over + L and (z < over + t or z >= over + L - t) and over <= x < over + W:
                for y in range(wall_h, roof_y - 2):
                    grid[(x, y, z)] = "daub_wall"
    return (X, Y, Z), grid


def encode(size, grid):
    X, Y, Z = size
    palette = [""]
    index = {"": 0}
    runs = []
    for y in range(Y):
        for z in range(Z):
            for x in range(X):
                m = grid.get((x, y, z), "")
                if m not in index:
                    index[m] = len(palette)
                    palette.append(m)
                i = index[m]
                if runs and runs[-1][0] == i:
                    runs[-1][1] += 1
                else:
                    runs.append([i, 1])
    return {"version": 1, "size": [X, Y, Z], "palette": palette, "runs": runs}


os.makedirs(OUT, exist_ok=True)
for name, args in (("longhouse", (12.0, 5.0, 2.0, 5.0)), ("small longhouse", (7.0, 4.0, 1.75, 4.0))):
    size, grid = longhouse(*args)
    path = os.path.join(OUT, name + ".json")
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write(json.dumps(encode(size, grid)) + "\n")
    print("wrote", path, size, len(grid), "voxels")
