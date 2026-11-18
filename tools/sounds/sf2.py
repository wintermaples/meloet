"""Read a SoundFont 2 file (RIFF sfbk): its samples and the zones a preset plays.

python3 -I tools/sounds/sf2.py <file.sf2> [--list] [--preset <bank>:<program> ...]

The file is data only. Only numpy is used. The rules follow the SoundFont 2.01 specification
(E-mu/Creative, "SoundFont Technical Specification 2.01"); the sections are named below.
"""

import argparse
import json
import struct

import numpy as np

# 8.1.2 generator enumerators (the ones this tool names)
GEN = {
    "startAddrsOffset": 0, "endAddrsOffset": 1, "startloopAddrsOffset": 2, "endloopAddrsOffset": 3,
    "startAddrsCoarseOffset": 4, "modLfoToPitch": 5, "vibLfoToPitch": 6, "modEnvToPitch": 7,
    "initialFilterFc": 8, "initialFilterQ": 9, "modLfoToFilterFc": 10, "modEnvToFilterFc": 11,
    "endAddrsCoarseOffset": 12, "modLfoToVolume": 13, "chorusEffectsSend": 15, "reverbEffectsSend": 16,
    "pan": 17, "delayModLFO": 21, "freqModLFO": 22, "delayVibLFO": 23, "freqVibLFO": 24,
    "delayModEnv": 25, "attackModEnv": 26, "holdModEnv": 27, "decayModEnv": 28, "sustainModEnv": 29,
    "releaseModEnv": 30, "keynumToModEnvHold": 31, "keynumToModEnvDecay": 32, "delayVolEnv": 33,
    "attackVolEnv": 34, "holdVolEnv": 35, "decayVolEnv": 36, "sustainVolEnv": 37, "releaseVolEnv": 38,
    "keynumToVolEnvHold": 39, "keynumToVolEnvDecay": 40, "instrument": 41, "keyRange": 43,
    "velRange": 44, "startloopAddrsCoarseOffset": 45, "keynum": 46, "velocity": 47,
    "initialAttenuation": 48, "endloopAddrsCoarseOffset": 50, "coarseTune": 51, "fineTune": 52,
    "sampleID": 53, "sampleModes": 54, "scaleTuning": 56, "exclusiveClass": 57, "overridingRootKey": 58,
}
NAME = {v: k for k, v in GEN.items()}
RANGES = (GEN["keyRange"], GEN["velRange"])

# 8.1.3 defaults of the generators that are not 0
DEFAULT = {
    "initialFilterFc": 13500, "delayModLFO": -12000, "delayVibLFO": -12000, "delayModEnv": -12000,
    "attackModEnv": -12000, "holdModEnv": -12000, "decayModEnv": -12000, "releaseModEnv": -12000,
    "delayVolEnv": -12000, "attackVolEnv": -12000, "holdVolEnv": -12000, "decayVolEnv": -12000,
    "releaseVolEnv": -12000, "keynum": -1, "velocity": -1, "scaleTuning": 100, "overridingRootKey": -1,
}

# 8.5 (p. 58): generators that are not used at the preset level (sample addresses, keynum, velocity,
# sampleModes, exclusiveClass, overridingRootKey); a preset zone that holds them is read without them.
NOT_IN_PRESET = {0, 1, 2, 3, 4, 12, 45, 46, 47, 50, 54, 57, 58}

# 7.10 sampleType
MONO, RIGHT, LEFT, LINKED = 1, 2, 4, 8
ROM = 0x8000


def _chunks(data, start, end):
    """(id, offset of the body, size) of the RIFF chunks in data[start:end]."""
    pos = start
    while pos + 8 <= end:
        cid, size = struct.unpack_from("<4sI", data, pos)
        yield cid.decode("latin-1"), pos + 8, size
        pos += 8 + size + (size & 1)


def _name(b):
    return b.split(b"\0", 1)[0].decode("latin-1").strip()


class SoundFont:
    def __init__(self, path):
        with open(path, "rb") as f:
            data = f.read()
        riff, size, form = struct.unpack_from("<4sI4s", data, 0)
        if riff != b"RIFF" or form != b"sfbk":
            raise ValueError("not a SoundFont 2 file (RIFF sfbk)")
        lists = {}
        for cid, off, sz in _chunks(data, 12, min(len(data), 8 + size)):
            if cid == "LIST":
                lists[data[off: off + 4].decode("latin-1")] = (off + 4, off + sz)
        # 5.1 INFO, 6.1 sdta (smpl: 16-bit samples; sm24, the low byte of 24-bit samples, is not read)
        self.info = {}
        a, z = lists["INFO"]
        for cid, off, sz in _chunks(data, a, z):
            self.info[cid] = _name(data[off: off + sz]) if cid not in ("ifil", "iver") \
                else "%d.%d" % struct.unpack_from("<HH", data, off)
        a, z = lists["sdta"]
        self.smpl = None
        for cid, off, sz in _chunks(data, a, z):
            if cid == "smpl":
                self.smpl = np.frombuffer(data, dtype="<i2", count=sz // 2, offset=off)
        if self.smpl is None:
            raise ValueError("no smpl chunk")
        # 7.1 the nine sub-chunks of pdta, records of fixed size
        a, z = lists["pdta"]
        pd = {cid: data[off: off + sz] for cid, off, sz in _chunks(data, a, z)}
        self.phdr = [(_name(r[0]), r[1], r[2], r[3]) for r in struct.iter_unpack("<20sHHHIII", pd["phdr"])]
        self.pbag = list(struct.iter_unpack("<HH", pd["pbag"]))
        self.pmod = list(struct.iter_unpack("<HHhHH", pd["pmod"]))
        self.pgen = list(struct.iter_unpack("<HH", pd["pgen"]))
        self.inst = [(_name(r[0]), r[1]) for r in struct.iter_unpack("<20sH", pd["inst"])]
        self.ibag = list(struct.iter_unpack("<HH", pd["ibag"]))
        self.imod = list(struct.iter_unpack("<HHhHH", pd["imod"]))
        self.igen = list(struct.iter_unpack("<HH", pd["igen"]))
        self.samples = []
        # 7.10: the last record is the terminal "EOS"
        for r in list(struct.iter_unpack("<20sIIIIIBbHH", pd["shdr"]))[:-1]:
            self.samples.append({"name": _name(r[0]), "start": r[1], "end": r[2], "loopStart": r[3],
                                 "loopEnd": r[4], "rate": r[5], "originalPitch": r[6],
                                 "pitchCorrection": r[7], "link": r[8], "type": r[9]})

    # ------------------------------------------------------------ zones

    @staticmethod
    def _gen_value(oper, raw):
        """keyRange and velRange are two bytes (lo, hi); the others are signed 16 bit (8.1.1)."""
        if oper in RANGES:
            return (raw & 0xFF, raw >> 8)
        return raw - 0x10000 if raw >= 0x8000 else raw

    def _zones(self, bags, gens, mods, first, last, terminal):
        """Zones of one preset or instrument: list of (generators {oper: value}, modulator count).
        7.3 / 7.7: if the first of several zones does not end with the terminal generator
        (instrument / sampleID), it is the global zone; it is returned as (None, gens, mods)."""
        zones = []
        for b in range(first, last):
            g0, m0 = bags[b]
            g1, m1 = bags[b + 1]
            g = {}
            order = []
            for oper, raw in gens[g0:g1]:
                g[oper] = self._gen_value(oper, raw)
                order.append(oper)
            zones.append((g, order, mods[m0:m1]))
        glob = None
        if len(zones) > 1 and (not zones[0][1] or zones[0][1][-1] != terminal):
            glob = zones.pop(0)
        # 7.3 / 7.7: a local zone without its terminal generator is ignored
        local = [z for z in zones if terminal in z[0]]
        return glob, local

    def find_preset(self, bank, program):
        for i, (name, prog, bk, _) in enumerate(self.phdr[:-1]):
            if prog == program and bk == bank:
                return i
        raise KeyError(f"no preset {bank}:{program}")

    def regions(self, bank, program):
        """Every (preset zone x instrument zone) a preset plays, after the rules of 8.5 and 9.4:
        - instrument level (9.4, 7.9): a local generator replaces the instrument's global one; one that
          neither sets has the default (8.1.3);
        - preset level (8.5, 9.4): a local generator replaces the preset's global one; the result is added
          to the instrument value (relative), except keyRange and velRange, which narrow the
          instrument's ranges (the zone plays where both answer).
        Returns (preset name, list of {"sample", "inst", "keyLo", "keyHi", "velLo", "velHi",
        "gen": {name: value}, "presetGen": {name: value}, "mods": n}), modulator records)."""
        pi = self.find_preset(bank, program)
        pname, _, _, pb0 = self.phdr[pi]
        pb1 = self.phdr[pi + 1][3]
        pglob, plocal = self._zones(self.pbag, self.pgen, self.pmod, pb0, pb1, GEN["instrument"])
        out = []
        mods = {"preset": [], "instrument": []}
        if pglob:
            mods["preset"] += pglob[2]
        for pg_local, _, pm in plocal:
            mods["preset"] += pm
            pg = dict(pglob[0]) if pglob else {}
            pg.update(pg_local)
            ii = pg[GEN["instrument"]]
            iname, ib0 = self.inst[ii]
            ib1 = self.inst[ii + 1][1]
            iglob, ilocal = self._zones(self.ibag, self.igen, self.imod, ib0, ib1, GEN["sampleID"])
            if iglob:
                mods["instrument"] += iglob[2]
            for ig_local, _, im in ilocal:
                mods["instrument"] += im
                ig = dict(iglob[0]) if iglob else {}
                ig.update(ig_local)
                pk = pg.get(GEN["keyRange"], (0, 127))
                pv = pg.get(GEN["velRange"], (0, 127))
                ik = ig.get(GEN["keyRange"], (0, 127))
                iv = ig.get(GEN["velRange"], (0, 127))
                klo, khi = max(pk[0], ik[0]), min(pk[1], ik[1])
                vlo, vhi = max(pv[0], iv[0]), min(pv[1], iv[1])
                if klo > khi or vlo > vhi:
                    continue
                gen = {}
                for name, num in GEN.items():
                    if num in RANGES or num in (GEN["instrument"], GEN["sampleID"]):
                        continue
                    v = ig.get(num, DEFAULT.get(name, 0))
                    if num not in NOT_IN_PRESET:
                        v += pg.get(num, 0)
                    gen[name] = v
                out.append({
                    "sample": ig[GEN["sampleID"]], "inst": iname,
                    "keyLo": klo, "keyHi": khi, "velLo": vlo, "velHi": vhi, "gen": gen,
                    "presetGen": {NAME.get(k, str(k)): v for k, v in pg.items() if k != GEN["instrument"]},
                    "instGen": {NAME.get(k, str(k)): v for k, v in ig.items() if k != GEN["sampleID"]},
                })
        return pname, out, mods

    def sample_points(self, region):
        """Sample start, end, loop start and loop end (absolute indexes into smpl) of a region,
        with the address offset generators added (8.1.2: 0..4, 12, 45, 50; coarse = 32768)."""
        s = self.samples[region["sample"]]
        g = region["gen"]
        start = s["start"] + g["startAddrsOffset"] + 32768 * g["startAddrsCoarseOffset"]
        end = s["end"] + g["endAddrsOffset"] + 32768 * g["endAddrsCoarseOffset"]
        ls = s["loopStart"] + g["startloopAddrsOffset"] + 32768 * g["startloopAddrsCoarseOffset"]
        le = s["loopEnd"] + g["endloopAddrsOffset"] + 32768 * g["endloopAddrsCoarseOffset"]
        return start, end, ls, le

    def pcm(self, start, end):
        return self.smpl[start:end]


def timecents_ms(tc):
    """8.1.3: envelope times are in timecents, seconds = 2^(tc/1200)."""
    return 1000.0 * 2.0 ** (tc / 1200.0)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("sf2")
    ap.add_argument("--list", action="store_true")
    ap.add_argument("--preset", action="append", default=[])
    a = ap.parse_args()
    sf = SoundFont(a.sf2)
    if a.list:
        print(json.dumps(sf.info, ensure_ascii=False))
        for name, prog, bank, _ in sf.phdr[:-1]:
            print(f"{bank:3d}:{prog:3d}  {name}")
    for p in a.preset:
        bank, prog = (int(x) for x in p.split(":"))
        pname, regs, mods = sf.regions(bank, prog)
        print(f"== {bank}:{prog} {pname}: {len(regs)} zones, modulators preset {len(mods['preset'])} "
              f"instrument {len(mods['instrument'])}")
        for r in regs:
            s = sf.samples[r["sample"]]
            print(json.dumps({"sample": s["name"], "type": s["type"], "link": s["link"], "rate": s["rate"],
                              "root": s["originalPitch"], "corr": s["pitchCorrection"],
                              "len": s["end"] - s["start"], "loop": [s["loopStart"] - s["start"], s["loopEnd"] - s["start"]],
                              "inst": r["inst"], "keys": [r["keyLo"], r["keyHi"]], "vel": [r["velLo"], r["velHi"]],
                              "instGen": r["instGen"], "presetGen": r["presetGen"]}, ensure_ascii=False))


if __name__ == "__main__":
    main()
