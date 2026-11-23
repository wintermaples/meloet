// The fake's part of protocol 33 (round 12, waves 3 and 4). Until each feature is built, the fake takes its intents
// and changes nothing, as Rust does (`// round12 stub: <ID>`).
import type { Arp } from "./types";

/** E2: the arpeggio a track starts with (off). */
export const FAKE_ARP_OFF: Arp = { on: false, shape: "up", rate: "sixteenth", triplet: false, octaves: 1, hold: false };
