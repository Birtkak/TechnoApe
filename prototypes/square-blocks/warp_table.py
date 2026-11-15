# The conformal grid as a warp of the equi-angular face, for the in-game trial:
#   position(a, b) = equi_angular(a + da(a, b), b + db(a, b)),   a and b in [-1, 1] across a face
# Writes warp_table.bin: K + 1 = 193 points a face edge, [ia][ib] pairs (da, db) of little-endian f32, bilinear
# between the points. The same table serves every face and every planet. Made exactly symmetric under the
# square's 8 symmetries, with no move across an edge, so two faces lay their shared edge on the same points.
#   python -I warp_table.py
import os
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
D = np.load(os.path.join(HERE, "conformal_face.npy"))
K = D.shape[0] - 1
c = np.linspace(-1, 1, K + 1)
A, B = np.meshgrid(c, c, indexing="ij")
p = np.stack([np.ones_like(A), np.tan(A * np.pi / 4), np.tan(B * np.pi / 4)], -1)
p = p / np.linalg.norm(p, axis=-1, keepdims=True) + D
da = np.arctan(p[..., 1] / p[..., 0]) / (np.pi / 4) - A
db = np.arctan(p[..., 2] / p[..., 0]) / (np.pi / 4) - B
raw = np.stack([da, db], -1)
# da is odd in a and even in b, and db is da mirrored in the diagonal
da = (da - da[::-1, :] + da[:, ::-1] - da[::-1, ::-1] + (db - db[:, ::-1] + db[::-1, :] - db[::-1, ::-1]).T) / 8
da[0, :] = da[-1, :] = 0.0
out = np.stack([da, da.T], -1).astype("<f4")
print("largest move %.4f of a half face, symmetrising changed it by at most %.2e" % (np.abs(out).max(), np.abs(out - raw).max()))
# the check: the warped lines never cross (a + da rises with a), so the map can be turned back
assert (np.diff(A + out[..., 0], axis=0) > 0).all()
assert (out[..., 0] == -out[::-1, :, 0]).all() and (out[..., 0] == out[:, ::-1, 0]).all() and (out[..., 1] == out[..., 0].T).all()
out.tofile(os.path.join(HERE, "warp_table.bin"))
# and the block it gives 100 columns in from a corner on the 4 km planet: 0.43 m, as measure.py's conformal grid
N, R = 6144, 6144 / np.sqrt(2 * np.pi / 3)


def point(a, b):
    x, y = (a + 1) * 0.5 * K, (b + 1) * 0.5 * K
    i, j = min(int(x), K - 1), min(int(y), K - 1)
    fx, fy = x - i, y - j
    d = out[i, j] * (1 - fx) * (1 - fy) + out[i + 1, j] * fx * (1 - fy) + out[i, j + 1] * (1 - fx) * fy + out[i + 1, j + 1] * fx * fy
    q = np.array([1.0, np.tan((a + d[0]) * np.pi / 4), np.tan((b + d[1]) * np.pi / 4)])
    return q / np.linalg.norm(q)


a = 1 - 200 / N
side = np.linalg.norm(point(a + 2 / N, a) - point(a, a)) * R
print("block 100 columns from a corner: %.3f m" % side)
assert abs(side - 0.433) < 0.01, side
