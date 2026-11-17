"""Costruisce src-tauri/resources/flybrain.bin: la parte del cervello che si attiva con i sensi usati
dal gioco (tutti i neuroni che sparano almeno una volta in una serie di stimoli, con le loro sinapsi).

  pip install pyarrow pandas scipy
  FLY_DATA=cartella_dati python3 tools/flybrain/build.py

Un neurone che non spara mai non influenza gli altri: tenerli solo quelli attivi cambia pochissimo
la dinamica e rende il file piccolo (qualche MB invece di centinaia).
"""
import itertools, os, struct, sys
import numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from connectome import load, groups, sel
from sim import run

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "../../src-tauri/resources/flybrain.bin")

# uscite: tipi cellulari scelti guardando quali neuroni discendenti/motori rispondono in modo
# specifico a ciascun senso (vedi README)
OUTPUTS = {
    "escape": ["DNp01", "DNp02", "DNp04", "DNp06", "DNp11"],  # Giant Fiber e compagni: fuga
    "backward": ["MDN"],  # moonwalker: indietreggia
    "groom": ["DNge132", "DNg85", "DNge122", "DNg48", "DNge067", "DNge121", "DNge011", "DNge012"],
    "feed": ["CB0700", "MNx01", "CB0911", "DNge031", "DNge059"],  # proboscide
    "drink": ["DNg67", "DNp58"],
    "listen": ["DNg24", "DNg29", "DNge113"],
    "squint": ["DNpe017", "DNp20"],
    "brace": ["DNg11", "DNp40", "DNg07", "DNb06"],
}


def main():
    ids, W, ann = load()
    S, O = groups(ann)
    outs = {k: sel(ann, cell_type=v) for k, v in OUTPUTS.items()}
    outs["turn_l"], outs["turn_r"] = O["turn_l"], O["turn_r"]
    names = list(S)
    default = ["pain", "loom_l", "loom_r", "sweet", "touch"]
    conds = [{n: 1.0} for n in names] + [{n: 0.3} for n in names]
    conds += [{a: 1.0, b: 1.0} for a, b in itertools.combinations(default, 2)]
    conds += [{n: 0.5 for n in names}, {n: 1.0 for n in names}]
    active = np.zeros(W.shape[0], bool)
    for i, c in enumerate(conds):
        stim = {tuple(S[n]): r for n, r in c.items()}
        r = run(W, stim, ms=600, seed=i)
        active |= r > 0
        print(i, c if len(c) < 3 else f"{len(c)} sensi", int(active.sum()), flush=True)
    keep = active.copy()
    for v in list(S.values()) + list(outs.values()):
        keep[v] = True
    k = np.flatnonzero(keep)
    remap = -np.ones(W.shape[0], np.int64)
    remap[k] = np.arange(len(k))
    sub = W[k][:, k].tocsr()
    sub.sort_indices()
    n, e = sub.shape[0], sub.nnz
    assert n < 65536
    w = np.clip(np.round(sub.data), -32767, 32767).astype("<i2")
    with open(OUT, "wb") as f:
        f.write(b"AEFLY1")
        f.write(struct.pack("<II", n, e))
        f.write(sub.indptr.astype("<u4").tobytes())
        f.write(sub.indices.astype("<u2").tobytes())
        f.write(w.tobytes())
        allg = [("in:" + a, S[a]) for a in names] + [("out:" + a, v) for a, v in outs.items()]
        f.write(struct.pack("<H", len(allg)))
        for name, v in allg:
            b = name.encode()
            f.write(struct.pack("<B", len(b)) + b)
            f.write(struct.pack("<H", len(v)))
            f.write(remap[v].astype("<u2").tobytes())
    print(f"neuroni {n}, sinapsi {e}, file {os.path.getsize(OUT) / 1e6:.1f} MB")


if __name__ == "__main__":
    main()
