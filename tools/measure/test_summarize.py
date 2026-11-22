#!/usr/bin/env python3
"""Checks the key -> screen split of summarize.py on a small hand-made run.

usage: python3 -I tools/measure/test_summarize.py
"""
import os, sys

sys.dont_write_bytecode = True

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import summarize  # noqa: E402

# (Rust received at ms, Rust -> drawn ms or None). The clock offset is 1000 ms.
KEYS = [
    (0, 1.0),  # first key: spaced
    (300, 1.0),  # 300 after: spaced, bin 150–
    (303, 14.0),  # 3 after: a chord's 2nd key, waited
    (305, 12.0),  # 2 after: its 3rd key, waited
    (330, 20.0),  # 25 after: spaced, bin 16.7–33.3, lit at 36.7 > 33.3
    (400, 0.5),  # 70 after: spaced, bin 33.3–150
    (600, None),  # 200 after: never drawn, but still the key before the next one
    (616, 40.0),  # 16 after: soon, lit at 56.7 > 50
]


def screen():
    offset_us = 1_000_000
    keys = []
    for t_ms, d in KEYS:
        t_us = 5_000_000 + t_ms * 1000
        rust_ms = (t_us - offset_us) / 1000
        keys.append({
            "note": 60,
            "tUs": t_us,
            "recvMs": rust_ms,
            "drawMs": None if d is None else rust_ms + d,
        })
    keys.reverse()  # the order in the file does not matter; the times do
    return {"offsetUs": offset_us, "keys": keys}


def main():
    r = summarize.summarize(None, screen(), None)
    spaced = r["keyToDrawByGap"]["spaced"]
    soon = r["keyToDrawByGap"]["soonAfter"]
    assert spaced["keyToDrawMs"]["count"] == 4, spaced
    assert spaced["keyToDrawMs"]["max"] == 20.0, spaced
    assert spaced["shownWithinTwoFramesPercent"] == 75.0, spaced
    assert spaced["shownWithinThreeFramesPercent"] == 100.0, spaced
    assert spaced["lineFrames"] == 2 and spaced["lineMet"] is False, spaced
    assert soon["keyToDrawMs"]["count"] == 3, soon
    assert round(soon["shownWithinThreeFramesPercent"]) == 67, soon
    assert soon["lineFrames"] == 3 and soon["lineMet"] is False, soon

    bins = {b["gapMs"]: b for b in r["keyToDrawByGapBins"]}
    # The first key has no key before; it counts as 150– (and as spaced).
    assert [bins[n]["keyToDrawMs"]["count"] for n in bins] == [3, 1, 1, 2], bins
    assert bins["0–16.7"]["waitedOver2Ms"] == 3, bins
    assert bins["16.7–33.3"]["waitedOver2Ms"] == 1, bins
    assert bins["33.3–150"]["waitedOver2Ms"] == 0, bins
    assert bins["150–"]["keyToDrawMs"]["max"] == 1.0, bins

    # Without the slow keys, both lines are met.
    global KEYS
    KEYS = [k for k in KEYS if k[1] is None or k[1] < 20]
    r = summarize.summarize(None, screen(), None)
    assert r["keyToDrawByGap"]["spaced"]["lineMet"] is True, r["keyToDrawByGap"]
    assert r["keyToDrawByGap"]["soonAfter"]["lineMet"] is True, r["keyToDrawByGap"]

    # No clock offset: nothing to split, and the table still prints.
    r = summarize.summarize(None, {"keys": screen()["keys"]}, None)
    assert r["keyToDrawByGap"]["spaced"]["lineMet"] is None
    summarize.markdown(r)

    # Callback -> DAC exactly one quantum (256 frames at 48 kHz: 5333.3 us) is marked, with
    # the keys it was; two quanta are not.
    records = []
    for n, gap in enumerate([10_667, 5_333, 5_334, 10_666, 5_333, 5_340]):
        callback = 2_000_000 + n * 257_000
        records.append({"keyUs": callback - 3_000, "callbackUs": callback, "playbackUs": callback + gap})
    sound = {"sampleRate": 48_000, "actualFrames": 256, "records": records}
    r = summarize.summarize(sound, None, None, measured_at="2026-11-22 18:21:05")
    once = r["callbackToDacOneQuantum"]
    assert once["keys"] == [2, 3, 5] and once["of"] == 6, once
    text = summarize.markdown(r)
    assert "for 3 of 6 keys (keys 2–3, 5), in the run of 2026-11-22 18:21:05" in text, text
    # Not marked when no key was, nor when the buffer is not known.
    sound["records"] = [records[0], records[3]]
    assert summarize.summarize(sound, None, None)["callbackToDacOneQuantum"]["count"] == 0
    assert "exactly one quantum (" not in summarize.markdown(summarize.summarize(sound, None, None))
    del sound["actualFrames"]
    assert summarize.summarize(sound, None, None)["callbackToDacOneQuantum"] is None
    print("summarize: ok")


if __name__ == "__main__":
    main()
