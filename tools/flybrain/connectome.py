"""Carica il connettoma FlyWire (v783, CC-BY 4.0) e le annotazioni; definisce i gruppi di neuroni.

Dati (non nel repository, troppo grandi):
  proofread_connections_783.feather  https://zenodo.org/records/10676866
  Supplemental_file1_neuron_annotations.tsv  https://github.com/flyconnectome/flywire_annotations
"""
import os, sys
import numpy as np
import pandas as pd
import pyarrow.feather as feather
from scipy import sparse

DATA = os.environ.get("FLY_DATA", os.path.dirname(os.path.abspath(__file__)) + "/data")

# segno per neurotrasmettitore (come Shiu et al. 2024: GABA e glutammato inibiscono).
# Dopamina, serotonina e octopamina sono modulatori lenti: niente effetto rapido.
NT = ["gaba_avg", "ach_avg", "glut_avg", "oct_avg", "ser_avg", "da_avg"]
SIGN = np.array([-1, 1, -1, 0, 0, 0], dtype=np.int8)


def load():
    ann = pd.read_csv(f"{DATA}/ann.tsv", sep="\t", low_memory=False)
    t = feather.read_table(f"{DATA}/conn.feather").to_pandas()
    nt = t[NT].to_numpy().argmax(1)
    t["w"] = t.syn_count * SIGN[nt]
    # le cellule di Kenyon sono colinergiche (la previsione dalle immagini spesso sbaglia)
    kc = set(ann.root_id[ann.cell_class.astype(str) == "Kenyon_Cell"])
    t.loc[t.pre_pt_root_id.isin(kc), "w"] = t.syn_count
    e = t.groupby(["pre_pt_root_id", "post_pt_root_id"], sort=False).w.sum().reset_index()
    e = e[e.w != 0]
    ids = np.unique(np.concatenate([e.pre_pt_root_id.values, e.post_pt_root_id.values, ann.root_id.values]))
    idx = {r: i for i, r in enumerate(ids)}
    pre = np.searchsorted(ids, e.pre_pt_root_id.values)
    post = np.searchsorted(ids, e.post_pt_root_id.values)
    W = sparse.csr_matrix((e.w.values.astype(np.float32), (pre, post)), shape=(len(ids), len(ids)))
    ann = ann.set_index("root_id").reindex(ids)
    W = prune(W, ann)
    return ids, W, ann


def prune(W, ann):
    """Toglie le sinapsi asso-assoniche tra cellule di Kenyon e tra neuroni sensoriali: in un modello
    LIF puramente eccitatorio innescano una "crisi" che coinvolge tutto il corpo a fungo."""
    kc = (ann.cell_class.astype(str) == "Kenyon_Cell").values
    sens = (ann.super_class.astype(str) == "sensory").values
    C = W.tocoo()
    drop = (kc[C.row] & kc[C.col]) | (sens[C.row] & sens[C.col])
    keep = ~drop
    return sparse.csr_matrix((C.data[keep], (C.row[keep], C.col[keep])), shape=W.shape)


def sel(ann, **kw):
    m = np.ones(len(ann), bool)
    for k, v in kw.items():
        col = ann[k].astype(str)
        if k == "side_of":
            continue
        m &= col.isin(v if isinstance(v, (list, tuple, set)) else [v]).values
    return np.flatnonzero(m)


def side(ann, ix, s):
    return ix[ann.side.values[ix] == s]


def groups(ann):
    """Sensi (ingressi) e uscite. Ogni sensore ha l'intensità 0..1 decisa dal gioco."""
    st = lambda **kw: sel(ann, **kw)
    loom = np.union1d(st(cell_type="LC4"), st(cell_type="LPLC2"))
    head_bristle = st(super_class="sensory", cell_sub_class="head bristle")
    head_bristle = head_bristle[ann.cell_type.values[head_bristle] != "BM_Taste"]
    S = {
        # predefiniti
        "pain": np.union1d(st(cell_type="TRN_VP2"), head_bristle),
        "loom_l": side(ann, loom, "left"),
        "loom_r": side(ann, loom, "right"),
        "sweet": st(super_class="sensory", cell_sub_class=["sugar", "sugar/low_salt"]),
        "touch": np.union1d(st(super_class="sensory", cell_sub_class="grooming"), st(cell_type="BM_Ant")),
        # opzionali
        "smell_food": st(cell_type=["ORN_DM1", "ORN_DM2", "ORN_DM4", "ORN_VA2", "ORN_DL5"]),
        "smell_danger": st(cell_type=["ORN_V", "ORN_DA2"]),
        "heat": st(cell_type="TRN_VP2"),
        "cold": st(cell_type=["TRN_VP3a", "TRN_VP3b", "HRN_VP1l"]),
        "wet": st(cell_type="HRN_VP5"),
        "water": st(super_class="sensory", cell_sub_class="water"),
        "sound": st(super_class="sensory", cell_sub_class="auditory"),
        "light": st(super_class="sensory", cell_sub_class="ocellar"),
        "wind": st(super_class="sensory", cell_sub_class="wind_gravity"),
    }
    dna02 = st(cell_type="DNa02")
    O = {
        "escape": st(cell_type=["DNp01", "DNp02", "DNp04", "DNp06", "DNp11"]),
        "giant_fiber": st(cell_type="DNp01"),
        "backward": st(cell_type="MDN"),
        "forward": st(cell_type="DNp09"),
        "turn_l": side(ann, dna02, "left"),
        "turn_r": side(ann, dna02, "right"),
    }
    return S, O


if __name__ == "__main__":
    ids, W, ann = load()
    print("neuroni", len(ids), "connessioni", W.nnz)
    S, O = groups(ann)
    for k, v in {**S, **O}.items():
        print(f"{k:14s} {len(v)}")
