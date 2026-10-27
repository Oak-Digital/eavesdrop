import { useEffect, useRef, useState } from "react";
import { HL } from "./hairline/recorder.generated.js";
import { vault } from "./hairline/vault.generated.js";

export function PrivacyVault() {
  const stageRef = useRef<HTMLDivElement>(null);
  const handleRef = useRef<ReturnType<typeof vault.mount> | null>(null);
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
    const handle = vault.mount({ stage, svg, read: { textContent: "rest" } }, vault.range[1]);
    handleRef.current = handle;
    return () => {
      handle.destroy();
      handleRef.current = null;
      svg.remove();
    };
  }, []);

  useEffect(() => {
    handleRef.current?.setPlaying(!reducedMotion);
  }, [reducedMotion]);

  return (
    <div className="privacy-illustration">
      <div
        ref={stageRef}
        className="vault-stage"
        data-hairline="vault"
        data-hairline-theme="light"
        role="img"
        aria-label="A closed personal vault with a gently turning combination dial"
      />
      <div className="vault-caption mono">
        <span>Your words. Under lock and key.</span>
      </div>
    </div>
  );
}
