import { useEffect, useRef, useState } from "react";
import { HL, recorder } from "./hairline/recorder.generated.js";

export function RecorderHero() {
  const stageRef = useRef<HTMLDivElement>(null);
  const readRef = useRef<HTMLSpanElement>(null);
  const equalizerRef = useRef<SVGPathElement>(null);
  const handleRef = useRef<ReturnType<typeof recorder.mount> | null>(null);
  const [playing, setPlaying] = useState(true);
  const [reducedMotion, setReducedMotion] = useState(
    () => window.matchMedia("(prefers-reduced-motion: reduce)").matches,
  );

  useEffect(() => {
    const query = window.matchMedia("(prefers-reduced-motion: reduce)");
    const update = () => setReducedMotion(query.matches);
    query.addEventListener("change", update);
    return () => query.removeEventListener("change", update);
  }, []);

  useEffect(() => {
    const stage = stageRef.current!;
    HL.inject(document);
    const svg = HL.mk("svg", { viewBox: "0 0 400 320", "aria-hidden": "true" }, stage);
    let time = "01:24";
    const read = {
      get textContent() { return time; },
      set textContent(value: string) {
        time = value;
        if (readRef.current) readRef.current.textContent = `${value} / 05:00`;
      },
    };
    const onFrame = (elapsed: number) => {
      // Decorative levels share the recorder's clock, pause state, and visibility.
      const bars = Array.from({ length: 65 }, (_, i) => {
        const x = 3 + (i / 64) * 594;
        const envelope = 0.45 + 0.55 * Math.sin((i / 64) * Math.PI);
        const signal = 0.5 + 0.3 * Math.sin(elapsed * 2.4 + i * 0.63)
          + 0.2 * Math.sin(elapsed * 3.7 - i * 0.37);
        const height = 3 + 18 * envelope * signal;
        return `M${x.toFixed(2)} ${(24 - height).toFixed(2)}V${(24 + height).toFixed(2)}`;
      });
      equalizerRef.current?.setAttribute("d", bars.join(""));
    };
    const handle = recorder.mount({ stage, svg, read, onFrame }, recorder.range[1]);
    handleRef.current = handle;
    return () => {
      handle.destroy();
      handleRef.current = null;
      svg.remove();
    };
  }, []);

  useEffect(() => {
    handleRef.current?.setPlaying(playing && !reducedMotion);
  }, [playing, reducedMotion]);

  return (
    <div className="hero-art">
      <div className="recorder-heading mono" aria-hidden="true">
        <span>Meeting audio</span>
        <span ref={readRef}>01:24 / 05:00</span>
      </div>
      <div
        ref={stageRef}
        className="recorder-stage"
        data-hairline="recorder"
        data-hairline-theme="light"
        role="img"
        aria-label="Tape recorder illustration"
        aria-description="Two tape reels turning. This illustration does not play or record audio."
      />
      <svg
        className="recorder-equalizer"
        viewBox="0 0 600 48"
        preserveAspectRatio="none"
        aria-hidden="true"
      >
        <path ref={equalizerRef} />
      </svg>
      <div className="recorder-caption mono">
        {!reducedMotion && (
          <button
            type="button"
            className="recorder-play"
            aria-label={playing ? "Pause recorder animation" : "Play recorder animation"}
            onClick={() => setPlaying((value) => !value)}
          >
            {playing ? "Pause" : "Play"}
            <svg width="10" height="10" viewBox="0 0 10 10" fill="currentColor" aria-hidden="true">
              {playing ? <path d="M2 1h2v8H2zm4 0h2v8H6z" /> : <path d="m2 1 7 4-7 4z" />}
            </svg>
          </button>
        )}
      </div>
    </div>
  );
}
