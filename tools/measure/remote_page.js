// What remote.py puts into the page once at the start: `k` in every `App.js()` body.
// Made-up input for the page alone; the real pointer and keyboard are not touched.
window.__remote = {
  all: (sel) => [...document.querySelectorAll(sel)],
  q(sel, i = 0) {
    const e = this.all(sel)[i];
    if (!e) throw new Error("no " + sel + " #" + i);
    return e;
  },
  click(sel, i = 0) {
    this.q(sel, i).click();
    return true;
  },
  texts(sel) {
    return this.all(sel).map((e) => e.innerText.replace(/\s+/g, " ").trim());
  },
  box(sel, i = 0) {
    const r = this.q(sel, i).getBoundingClientRect();
    return [r.left, r.top, r.width, r.height];
  },
  status() {
    const e = document.querySelector("footer.status");
    return e ? e.innerText.replace(/\s+/g, " ").trim() : null;
  },
  ptr(el, type, x, y, buttons = 0, more = {}) {
    const init = {
      bubbles: true, cancelable: true, composed: true, clientX: x, clientY: y, pointerId: 1,
      pointerType: "mouse", isPrimary: true, button: type === "pointermove" ? -1 : 0, buttons, ...more,
    };
    el.dispatchEvent(new PointerEvent(type, init));
  },
  sleep: (ms) => new Promise((r) => setTimeout(r, ms)),
  frame: () => new Promise((r) => requestAnimationFrame(r)),
  // A drag the way a hand does it: down on the middle of `el`, a move each frame, up. The
  // moves go to what is under the point, or to `el` itself with `same` (an element that
  // took the pointer for itself gets them wherever the pointer is).
  async drag(el, dx, dy, steps = 8, more = {}) {
    const { same, hold, ...init } = more;
    const r = el.getBoundingClientRect();
    const x = r.left + r.width / 2, y = r.top + r.height / 2;
    const under = (px, py) => (same ? el : document.elementFromPoint(px, py) || el);
    this.ptr(el, "pointerdown", x, y, 1, init);
    for (let i = 1; i <= steps; i++) {
      await this.frame();
      this.ptr(under(x + (dx * i) / steps, y + (dy * i) / steps), "pointermove", x + (dx * i) / steps, y + (dy * i) / steps, 1, init);
    }
    if (hold) await this.sleep(hold);
    await this.frame();
    this.ptr(under(x + dx, y + dy), "pointerup", x + dx, y + dy, 0, init);
    under(x + dx, y + dy).dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true, clientX: x + dx, clientY: y + dy }));
  },
  key(key, more = {}, el = null) {
    const to = el || document.activeElement || document.body;
    const init = { key, code: more.code || key, bubbles: true, cancelable: true, composed: true, ...more };
    const went = to.dispatchEvent(new KeyboardEvent("keydown", init));
    to.dispatchEvent(new KeyboardEvent("keyup", init));
    return went;
  },
  type(el, text) {
    el.focus();
    el.value = text;
    el.dispatchEvent(new Event("input", { bubbles: true }));
  },
  context(sel, i = 0) {
    const e = this.q(sel, i), r = e.getBoundingClientRect();
    e.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: r.left + 4, clientY: r.top + 4, button: 2 }));
  },
};
