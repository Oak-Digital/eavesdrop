import { useEffect, useRef, useState } from "react";
import { HL, recorder } from "./hairline/recorder.generated.js";

export function RecorderHero() {
  const stageRef = useRef<HTMLDivElement>(null);
  const readRef = useRef<HTMLSpanElement>(null);
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
    const read = {
      get textContent() { return readRef.current?.textContent ?? "rest"; },
      set textContent(value: string) {
        if (readRef.current) readRef.current.textContent = value === "rest" ? "01:24 / 05:00" : `${value} / 05:00`;
      },
    };
    const handle = recorder.mount({ stage, svg, read }, recorder.range[1]);
    return () => {
      handle.destroy();
      svg.remove();
    };
  }, []);

  useEffect(() => {
    if (!playing || reducedMotion) return;
    const tour = HL.tour(stageRef.current!, recorder.tour);
    return () => tour.stop();
  }, [playing, reducedMotion]);

  return (
    <div className="hero-art">
      <div className="recorder-heading mono" aria-hidden="true">
        <span>Every word has a place.</span>
        <span ref={readRef}>01:24 / 05:00</span>
      </div>
      <div
        ref={stageRef}
        className="recorder-stage"
        data-hairline="recorder"
        data-hairline-theme="light"
        role="slider"
        tabIndex={0}
        aria-label="Tape recorder illustration"
        aria-description="Move across the recorder or use the arrow keys to scrub the tape. This illustration does not play or record audio."
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={28}
      />
      <div className="recorder-caption mono">
        <span>Move across to scrub the tape</span>
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
