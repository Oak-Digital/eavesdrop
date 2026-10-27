/** A closed personal safe: the dial turns, then the handle seats the lock. */
const {
  Cam, fit, proj, rings, rrect, prism, poly, open, ringAt, clamp,
  spring, stepS, reducedMotion, mk, solid, put, pointer, register, disposer,
} = HL;

function mount({ stage, svg, read }, value) {
  const bag = disposer(), C = Cam(45, 0.5, 1.85);
  fit(C, [[-52, -32, 0], [52, -32, 0], [-52, 43, 0], [52, 43, 0],
    [-52, -32, 104], [52, 43, 104]], 200, 166);
  const P = proj(C), g = mk("g", {}, svg);
  // Upright rounded rings extrude towards the viewer, along world y.
  const Q = (u, v, depth) => P(u, depth, v);
  const front = (q) => q.nu * Math.SQRT1_2 + q.nv * Math.sqrt(3) / 2 > 0;
  let reach = value, elapsed = 0, playing = false, enabled = true, hover = null;
  const angle = spring(0.28), seat = spring(0);
  let lastAngle = NaN, lastSeat = NaN;

  function block(x0, z0, x1, z1, radius, back, face) {
    const [outer, inner] = rings(x0, z0, x1, z1, radius, 0.8);
    const body = solid(g);
    put(body, prism(Q, front, outer, inner, back, face));
    return body;
  }
  function round(cx, cz, r, back, face) {
    const outer = rrect(cx - r, cz - r, cx + r, cz + r, r, 16);
    const inner = rrect(cx - r + 0.7, cz - r + 0.7, cx + r - 0.7, cz + r - 0.7, r - 0.7, 16);
    const body = solid(g);
    put(body, prism(Q, front, outer, inner, back, face));
    return body;
  }
  // Feet, enclosure, recessed door seam, and the thick closed door.
  block(-43, 0, -26, 9, 3, -23, 22);
  block(28, 0, 45, 9, 3, -23, 22);
  block(-52, 7, 52, 104, 7, -32, 27);
  mk("path", { d: poly(ringAt(Q, rrect(-45, 14, 45, 97, 5, 8), 27.2)), class: "nf lo" }, g);
  block(-41, 18, 41, 93, 4.5, 27.5, 31);
  // Two substantial hinges, each with a real barrel seam.
  [28, 76].forEach((z) => {
    block(37, z, 46, z + 15, 4, 28, 36);
    mk("path", { d: open([Q(38, z + 7.5, 36.2), Q(45, z + 7.5, 36.2)]), class: "nf lo" }, g);
  });
  round(-10, 62, 18, 31, 33);
  const ticks = [];
  for (let i = 0; i < 24; i++) {
    const a = i * Math.PI / 12, r = i % 3 === 0 ? 14.6 : 15.8;
    ticks.push(open([Q(-10 + r * Math.sin(a), 62 + r * Math.cos(a), 33.2),
      Q(-10 + 17 * Math.sin(a), 62 + 17 * Math.cos(a), 33.2)]));
  }
  mk("path", { d: ticks.join(""), class: "nf lo" }, g);
  const dial = round(-10, 62, 11.5, 33, 38);
  dial.sil.classList.add("hi");
  const dialMarks = mk("path", { class: "nf sil" }, g);
  round(14, 36, 5, 31, 34);
  const handle = solid(g);

  function draw() {
    if (angle.x !== lastAngle) {
      lastAngle = angle.x;
      const marks = [];
      for (let i = 0; i < 12; i++) {
        const a = angle.x + i * Math.PI / 6;
        marks.push(open([Q(-10 + 9 * Math.sin(a), 62 + 9 * Math.cos(a), 38.2),
          Q(-10 + 10.5 * Math.sin(a), 62 + 10.5 * Math.cos(a), 38.2)]));
      }
      marks.push(open([Q(-10 + 3 * Math.sin(angle.x), 62 + 3 * Math.cos(angle.x), 38.2),
        Q(-10 + 7 * Math.sin(angle.x), 62 + 7 * Math.cos(angle.x), 38.2)]));
      dialMarks.setAttribute("d", marks.join(""));
    }
    if (seat.x !== lastSeat) {
      lastSeat = seat.x;
      const [outer, inner] = rings(10.5, 23, 17.5, 41, 3.5, 0.7);
      put(handle, prism(Q, front, outer, inner, 34, 39 - seat.x));
    }
  }
  const B = register(stage, (dt) => {
    if (!enabled) return false;
    const ambient = playing && !reducedMotion() && hover === null;
    if (ambient) elapsed += dt;
    // A long, eased sweep followed by a small inward locking movement.
    const phase = elapsed * Math.PI * 2 / 12;
    angle.t = 0.28 + (hover === null ? (ambient ? Math.sin(phase) * reach : 0) : hover * reach);
    seat.t = hover === null && ambient ? Math.pow((1 - Math.cos(phase)) / 2, 5) * 1.8 : 0;
    const turning = stepS(angle, dt), seating = stepS(seat, dt);
    draw();
    return ambient || turning || seating;
  });
  bag.add(B.unregister);
  const center = P(-10, 38, 62);
  bag.add(pointer(stage, {
    move: ([x, y]) => {
      if (!enabled) return;
      hover = Math.hypot((x - center[0]) / 1.2, y - center[1]) < 48
        ? clamp((x - center[0]) / 35, -1, 1) : null;
      read.textContent = hover === null ? "rest" : "dial";
      B.wake();
    },
    leave: () => { hover = null; read.textContent = "rest"; B.wake(); },
  }));
  bag.add(() => svg.replaceChildren());
  read.textContent = "rest";
  draw();
  return {
    set: (v) => { reach = clamp(v, 0.3, 1.2); B.wake(); },
    setPlaying: (on) => { playing = on; enabled = on; hover = null; read.textContent = "rest"; B.wake(); },
    destroy: bag.dispose,
  };
}

hairline({
  name: "vault",
  means: "A closed personal vault: turn its mechanical dial while your recordings stay safely inside.",
  rules: [1, 3, 5, 7, 8, 9],
  range: [0.3, 0.7, 1.2],
  tour: [[125, 172], [175, 172], [150, 172], null],
  mount,
});
