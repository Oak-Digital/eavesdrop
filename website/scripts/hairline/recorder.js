/** A tape deck with continuously turning reels, independent of the pointer. */
const {
  Cam, facing, fit, proj, rings, rrect, prism, poly, open, ringAt,
  reducedMotion, mk, solid, put, register, disposer,
} = HL;

function mount({ stage, svg, read, onFrame }, value) {
  const bag = disposer(), C = Cam(45, 0.5, 1.72);
  fit(C, [[-76, -56, 0], [76, -56, 0], [-76, 56, 0], [76, 56, 0], [-76, -56, 27]], 200, 166);
  const P = proj(C), front = facing(C), g = mk("g", {}, svg);
  let speed = value, playing = true, elapsed = 84, phase = 0.392, drawn = NaN;

  function block(x0, y0, x1, y1, radius, bottom, top, parent = g) {
    const [ring, inner] = rings(x0, y0, x1, y1, radius, 0.8);
    const el = solid(parent);
    put(el, prism(P, front, ring, inner, bottom, top));
    return el;
  }
  function round(cx, cy, radius, z) {
    return poly(ringAt(P, rrect(cx - radius, cy - radius, cx + radius, cy + radius, radius, 16), z));
  }
  // A rounded enclosure and its inset face, then the transport from back to front.
  block(-76, -56, 76, 56, 9, 0, 15);
  mk("path", { d: poly(ringAt(P, rrect(-70, -50, 70, 50, 6, 6), 15)), class: "nf lo" }, g);
  const tape = mk("path", { class: "nf sil" }, g);
  const reels = [-36, 36].map((cx) => {
    const group = mk("g", {}, g);
    const ring = rrect(cx - 29, -44, cx + 29, 14, 29, 16);
    const inner = rrect(cx - 28, -43, cx + 28, 13, 28, 16);
    const body = solid(group);
    put(body, prism(P, front, ring, inner, 18, 22));
    const winding = mk("path", { class: "nf lo" }, group);
    const spokes = mk("path", { class: "nf sil" }, group);
    mk("path", { d: round(cx, -15, 5.5, 22.5), class: "sil" }, group);
    mk("path", { d: round(cx, -15, 1.6, 23), class: "nf lo" }, group);
    return { cx, body, winding, spokes };
  });
  // Tape guides, the recording head, and four mechanical transport keys.
  [-32, 32].forEach((x) => block(x - 3, 24, x + 3, 30, 3, 15, 21));
  block(-12, 25, 12, 36, 2.5, 15, 22);
  mk("path", { d: open([P(-8, 33, 22), P(8, 33, 22)]), class: "nf lo" }, g);
  for (let i = 0; i < 4; i++) block(-26 + i * 14, 41, -15 + i * 14, 49, 2, 15, i === 1 ? 16.5 : 19);

  // The fixed tape pack gives the repeating animation a seamless rest pose.
  const radii = [Math.sqrt(560 - 0.28 * 380), Math.sqrt(180 + 0.28 * 380)];
  tape.setAttribute("d", open([
    P(-36 - radii[0], -15, 19), P(-35, 27, 19),
    P(35, 27, 19), P(36 + radii[1], -15, 19),
  ]));
  reels.forEach((reel, i) => {
    reel.winding.setAttribute("d", [radii[i], radii[i] - 1.8].map((r) => round(reel.cx, -15, r, 22.2)).join(""));
  });
  const slot = rrect(9, -3.5, 25, 3.5, 3.5, 6);
  function draw() {
    if (phase === drawn) return;
    drawn = phase;
    reels.forEach((reel, i) => {
      const angle = phase * Math.PI * 2 + i * 0.8;
      // Three rounded radial windows make the rotation readable even at thumbnail size.
      const windows = [];
      for (let arm = 0; arm < 3; arm++) {
        const a = angle + arm * Math.PI * 2 / 3;
        windows.push(poly(slot.map(({ u, v }) => P(
          reel.cx + u * Math.cos(a) - v * Math.sin(a),
          -15 + u * Math.sin(a) + v * Math.cos(a), 22.4,
        ))));
      }
      reel.spokes.setAttribute("d", windows.join(""));
    });
    const seconds = Math.floor(elapsed) % 300;
    const time = `${Math.floor(seconds / 60).toString().padStart(2, "0")}:${(seconds % 60).toString().padStart(2, "0")}`;
    if (read.textContent !== time) read.textContent = time;
    onFrame?.(elapsed);
  }
  const B = register(stage, (dt) => {
    if (!playing || reducedMotion()) return false;
    elapsed += dt;
    phase = (phase + dt * speed / 14) % 1;
    draw();
    return true;
  });
  bag.add(B.unregister);
  bag.add(() => svg.replaceChildren());
  reels[0].body.sil.classList.add("hi");
  draw();
  return {
    set: (v) => { speed = v; B.wake(); },
    setPlaying: (on) => { playing = on; if (on) B.wake(); },
    destroy: bag.dispose,
  };
}

hairline({
  name: "recorder",
  means: "A tape recorder with two reels turning continuously.",
  rules: [3, 5, 7, 9],
  range: [0.6, 1.4, 2.4],
  mount,
});
