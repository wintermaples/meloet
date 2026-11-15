// テイクのため場所の片づけ: 曲のどこからも指されていないテイクは手放し、新しい 16 個だけためておく。
import { describe, expect, it } from "vitest";
import { encodeTake } from "../src/bridge/fake";
import { KEEP_DROPPED_TAKES, songTakeIds, TakeStore } from "../src/bridge/take";

function store() {
  const fetched: number[] = [];
  const s = new TakeStore(async (id) => {
    fetched.push(id);
    // id が偶数のテイクは、1 つ前の id をもとのテイク（クオンタイズする前）に持つ。
    return encodeTake({ id, length: 1920, origin: id % 2 === 0 ? id - 1 : undefined, notes: [{ start: 0, length: 480, key: 60, velocity: 80 }] });
  });
  return { s, fetched };
}

describe("TakeStore.keepOnly", () => {
  it("1000 回 id を入れかえても、ためている数は一定のまま", async () => {
    const { s } = store();
    for (let id = 1; id <= 1000; id += 2) {
      await s.get(id);
      s.keepOnly([id]);
    }
    expect(s.held()).toEqual({ ready: 1, dropped: KEEP_DROPPED_TAKES });
  });

  it("使っているテイクと、そのもとのテイクは手放さない", async () => {
    const { s } = store();
    await s.get(3);
    await s.get(4);
    await s.get(5);
    s.keepOnly([4]);
    expect(s.held()).toEqual({ ready: 2, dropped: 1 });
    expect(s.peek(3)).toBeDefined();
  });

  it("取り消しで戻ったテイクは、ためてある物から取りなおさずに出す。ためてもいない物は Rust から取りなおす", async () => {
    const { s, fetched } = store();
    for (let id = 1; id <= 41; id += 2) {
      await s.get(id);
      s.keepOnly([id]);
    }
    fetched.length = 0;
    s.keepOnly([39]);
    await s.get(39);
    expect(fetched).toEqual([]);
    await s.get(1);
    expect(fetched).toEqual([1]);
  });

  it("曲のクリップが指しているテイクの id", () => {
    const clip = (takeId: number) => ({ takeId });
    expect(songTakeIds({ tracks: [{ clips: [clip(3), clip(7)] }, { clips: [] }, { clips: [clip(9)] }] })).toEqual([3, 7, 9]);
  });
});
