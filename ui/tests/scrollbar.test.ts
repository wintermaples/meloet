import { describe, expect, it } from "vitest";
import { dragTo, pageTo, thumbOf } from "../src/lib/scrollbar";

describe("scroll bar (spec 2)", () => {
  it("not shown when the content fits", () => {
    expect(thumbOf({ view: 300, content: 300, top: 0 }, 8)).toBeNull();
    expect(thumbOf({ view: 300, content: 300.5, top: 0 }, 8)).toBeNull();
    expect(thumbOf({ view: 0, content: 100, top: 0 }, 8)).toBeNull();
  });

  it("handle length is the visible ratio, position is the scroll position", () => {
    expect(thumbOf({ view: 300, content: 600, top: 0 }, 8)).toEqual({ length: 150, offset: 0 });
    expect(thumbOf({ view: 300, content: 600, top: 300 }, 8)).toEqual({ length: 150, offset: 150 });
    expect(thumbOf({ view: 100, content: 100000, top: 0 }, 8)!.length).toBe(8);
  });

  it("moving the handle moves the content by the same ratio. Stops at the ends", () => {
    const box = { view: 300, content: 600, top: 0 };
    expect(dragTo(box, 0, 75, 8)).toBe(150);
    expect(dragTo(box, 0, 1000, 8)).toBe(300);
    expect(dragTo(box, 100, -1000, 8)).toBe(0);
  });

  it("pressing outside the handle moves one screen that way", () => {
    expect(pageTo({ view: 300, content: 1000, top: 0 }, 250, 8)).toBe(300);
    expect(pageTo({ view: 300, content: 1000, top: 600 }, 0, 8)).toBe(300);
    expect(pageTo({ view: 300, content: 1000, top: 600 }, 299, 8)).toBe(700);
  });
});
