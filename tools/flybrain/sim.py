"""Simulazione LIF del cervello (parametri di Shiu et al., Nature 2024), guidata dagli eventi."""
import numpy as np

P = dict(v0=-52.0, vrst=-52.0, vth=-45.0, tmbr=20.0, tau=5.0, trfc=2.2, tdly=1.8, wsyn=0.275, rpoi=150.0, fpoi=250.0)
DT = 0.5  # ms


def run(W, stim, ms=1000, seed=0, dt=DT):
    """stim: {indici: intensità 0..1}. Ritorna il numero di spike per neurone."""
    rng = np.random.default_rng(seed)
    N = W.shape[0]
    Wt = W.tocsr()
    v = np.full(N, P["v0"])
    x = np.zeros(N)
    ref = np.zeros(N)
    D = int(round(P["tdly"] / dt))
    ring = np.zeros((D + 1, N))
    count = np.zeros(N, np.int32)
    dv = dt / P["tmbr"]
    dx = np.exp(-dt / P["tau"])
    inputs = [(np.asarray(ix), r) for ix, r in stim.items() if r > 0]
    for step in range(int(ms / dt)):
        slot = step % (D + 1)
        x += ring[slot]
        ring[slot] = 0
        act = ref <= 0
        v[act] += dv * (x[act] - (v[act] - P["v0"]))
        x *= dx
        ref -= dt
        for ix, r in inputs:
            hit = ix[rng.random(len(ix)) < P["rpoi"] * r * dt / 1000]
            v[hit] += P["fpoi"] * P["wsyn"]
        sp = np.flatnonzero((v >= P["vth"]) & act)
        if len(sp):
            v[sp] = P["vrst"]
            ref[sp] = P["trfc"]
            count[sp] += 1
            tgt = (step + D) % (D + 1)
            row = Wt[sp].sum(0).A1 if len(sp) > 1 else Wt[sp].toarray()[0]
            ring[tgt] += row * P["wsyn"]
    return count * (1000.0 / ms)
