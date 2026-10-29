import { useRef, useState } from "react";
import { AsciiReveal, type AsciiRevealHandle } from "ascii-reveal";
import { Icon } from "./Icons";

const tabs = ["Summary", "Transcript", "Recording"] as const;
type Tab = (typeof tabs)[number];
export default function Demo() {
  const [tab, setTab] = useState<Tab>("Summary");
  const reveal = useRef<AsciiRevealHandle>(null);
  function choose(next: Tab) {
    setTab(next);
  }
  return (
    <div className="product-window">
      <div className="window-bar">
        <div className="window-dots" aria-hidden="true">
          <i />
          <i />
          <i />
        </div>
        <span>Eavesdrop</span>
        <span className="window-local">
          <Icon name="lock" size={12} /> Local library
        </span>
      </div>
      <div className="product-body">
        <aside className="demo-sidebar" aria-label="Example library">
          <div className="demo-brand">
            <Icon name="wave" size={22} gradient /> eavesdrop
          </div>
          <div className="sidebar-label">
            <Icon name="file" size={16} /> Recordings <span>3</span>
          </div>
          <div className="demo-meeting selected">
            <strong>Product catch-up</strong>
            <span>Today · 24 min</span>
          </div>
          <div className="demo-meeting">
            <strong>Client introduction</strong>
            <span>Yesterday · 38 min</span>
          </div>
          <div className="demo-meeting">
            <strong>Monday planning</strong>
            <span>Monday · 16 min</span>
          </div>
          <p className="sidebar-foot">
            <Icon name="lock" size={13} /> Stored on this computer
          </p>
        </aside>
        <div className="demo-main">
          <div className="demo-heading">
            <div>
              <p className="mono muted">Today, 10:00 · 24 min</p>
              <h3>Product catch-up</h3>
            </div>
            <span className="sample-label">Example recording</span>
          </div>
          <div
            className="demo-tabs"
            role="tablist"
            aria-label="Explore the recording"
          >
            {tabs.map((name, index) => (
              <button
                key={name}
                role="tab"
                id={`tab-${name}`}
                aria-controls="demo-panel"
                aria-selected={tab === name}
                tabIndex={tab === name ? 0 : -1}
                onClick={() => choose(name)}
                onKeyDown={(event) => {
                  let next: number | undefined;
                  if (event.key === "ArrowRight")
                    next = (index + 1) % tabs.length;
                  if (event.key === "ArrowLeft")
                    next = (index + tabs.length - 1) % tabs.length;
                  if (event.key === "Home") next = 0;
                  if (event.key === "End") next = tabs.length - 1;
                  if (next !== undefined) {
                    event.preventDefault();
                    choose(tabs[next]);
                    document.getElementById(`tab-${tabs[next]}`)?.focus();
                  }
                }}
              >
                {name}
              </button>
            ))}
            <button
              className="replay"
              aria-label="Replay ASCII reveal"
              onClick={() => reveal.current?.reveal()}
            >
              <Icon name="replay" size={14} />
              <span>Replay</span>
            </button>
          </div>
          <div
            role="tabpanel"
            id="demo-panel"
            aria-labelledby={`tab-${tab}`}
            tabIndex={0}
            className="demo-panel"
          >
            <AsciiReveal
              ref={reveal}
              key={tab}
              preset="decode"
              origin="top-left"
            >
              {tab === "Summary" && (
                <div className="summary-content">
                  <div className="summary-byline">
                    <Icon name="file" size={14} /> Concise summary
                  </div>
                  <h4>Simplifying the first recording</h4>
                  <p>
                    The team walked through the new onboarding flow. The
                    priority is to get people to their first recording with
                    fewer decisions along the way.
                  </p>
                  <h5>Decisions</h5>
                  <ul>
                    <li>
                      Keep the initial setup focused on microphone access.
                    </li>
                    <li>Introduce transcription after the first recording.</li>
                  </ul>
                  <h5>Next steps</h5>
                  <p className="task-line">
                    <Icon name="check" size={15} /> Alex to share the revised
                    flow before Thursday.
                  </p>
                </div>
              )}
              {tab === "Transcript" && (
                <div className="transcript-content">
                  <div className="summary-byline">
                    <Icon name="mic" size={14} /> Transcribed on this device
                  </div>
                  <h4>Transcript with timestamps</h4>
                  {[
                    [
                      "00:12",
                      "I think the first five minutes should feel really simple. What is the one thing someone needs to do?",
                    ],
                    [
                      "00:24",
                      "Make a recording. We can introduce transcription once they have something to work with.",
                    ],
                    [
                      "00:41",
                      "Agreed. Let’s keep setup focused on microphone access and leave the rest for later.",
                    ],
                    [
                      "01:08",
                      "I’ll share the revised flow before Thursday, so we can review it together.",
                    ],
                  ].map(([time, text]) => (
                    <p className="transcript-line" key={time}>
                      <span>{time}</span>
                      {text}
                    </p>
                  ))}
                </div>
              )}
              {tab === "Recording" && (
                <div className="recording-content">
                  <div className="summary-byline">
                    <Icon name="wave" size={14} /> Microphone + computer audio
                  </div>
                  <h4>Saved meeting audio</h4>
                  <div className="audio-wave" aria-hidden="true">
                    {Array.from({ length: 72 }, (_, index) => (
                      <i
                        key={index}
                        style={{
                          height: `${12 + Math.abs(Math.sin(index * 1.7) * Math.cos(index * 0.23)) * 64}px`,
                        }}
                      />
                    ))}
                  </div>
                  <div className="audio-timeline mono">
                    <span>00:00</span>
                    <span>24:08</span>
                  </div>
                  <p>
                    Listen to the recording in Eavesdrop, or export it as an
                    M4A audio file to share. The original audio stays encrypted
                    in the library on your computer.
                  </p>
                  <p className="recording-note">
                    Example only · no audio is recorded or played on
                    this page.
                  </p>
                </div>
              )}
            </AsciiReveal>
          </div>
        </div>
      </div>
    </div>
  );
}
