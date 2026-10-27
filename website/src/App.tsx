import {
  Component,
  lazy,
  Suspense,
  useEffect,
  useState,
  type ReactNode,
} from "react";
import { AsciiReveal, AsciiSkeleton } from "ascii-reveal";
import { Icon } from "./Icons";
import {
  parseRelease,
  RELEASE_API,
  RELEASES_URL,
  REPOSITORY,
} from "./releases.mjs";
import snapshot from "./release-snapshot.json";

const Demo = lazy(() => import("./Demo"));
const signal = Array.from({ length: 33 }, (_, y) =>
  Array.from({ length: 65 }, (_, x) => {
    const dx = (x - 32) / 32;
    const dy = (y - 16) / 16;
    const radius = Math.sqrt(dx * dx + dy * dy);
    const envelope = Math.exp(-dx * dx * 4.5);
    const wave =
      Math.abs(Math.sin(dx * 24) * 0.62 + Math.sin(dx * 57) * 0.22) * envelope;
    if (Math.abs(dy) < wave) return Math.abs(dy) < wave * 0.5 ? "█" : "▒";
    if (radius > 0.94 && radius < 1.01) return "+";
    if (y === 16) return "─";
    return (x + y * 3) % 9 === 0 && radius < 1 ? "·" : " ";
  }).join(""),
).join("\n");

class PreviewBoundary extends Component<
  { children: ReactNode },
  { failed: boolean }
> {
  state = { failed: false };
  static getDerivedStateFromError() {
    return { failed: true };
  }
  render() {
    return this.state.failed ? (
      <div className="preview-fallback">
        <Icon name="wave" size={32} />
        <p>The interactive preview could not load.</p>
        <button className="button" onClick={() => window.location.reload()}>
          Reload page
        </button>
      </div>
    ) : (
      this.props.children
    );
  }
}

function Downloads() {
  const [release, setRelease] = useState<{
    version: string;
    mac: string | null;
    windows: string | null;
    notes: string;
  } | null>(null);
  const [fallback, setFallback] = useState(false);
  useEffect(() => {
    const controller = new AbortController();
    let mounted = true;
    const timeout = window.setTimeout(() => controller.abort(), 6000);
    fetch(RELEASE_API, {
      signal: controller.signal,
      headers: { Accept: "application/vnd.github+json" },
    })
      .then((response) => {
        if (!response.ok) throw new Error("Release lookup failed");
        return response.json();
      })
      .then((data) => {
        if (mounted) setRelease(parseRelease(data));
      })
      .catch(() => {
        if (mounted) {
          setRelease(snapshot);
          setFallback(true);
        }
      })
      .finally(() => window.clearTimeout(timeout));
    return () => {
      mounted = false;
      window.clearTimeout(timeout);
      controller.abort();
    };
  }, []);
  return (
    <section
      className="downloads section"
      id="download"
      aria-labelledby="download-title"
    >
      <div className="section-intro">
        <h2 id="download-title">
          Make room for
          <br />
          the conversation.
        </h2>
        <p>
          Install Eavesdrop. Choose your microphone.
          <br />
          You’re ready for the next meeting.
        </p>
      </div>
      <div className="download-content" aria-busy={!release}>
        <p className="release-status mono" role="status">
          {release
            ? `${fallback ? "Available release" : "Latest release"} · ${release.version}`
            : "Finding the latest release…"}
        </p>
        <AsciiReveal loading={!release} preset="decode" revealOnMount={false}>
          <div className="download-options">
            {(
              [
                {
                  platform: "macOS",
                  icon: "apple",
                  detail: "macOS 13 or later",
                  architecture: "Apple silicon & Intel · .dmg",
                  url: release?.mac,
                },
                {
                  platform: "Windows",
                  icon: "windows",
                  detail: "Windows 10 / 11",
                  architecture: "64-bit · .msi",
                  url: release?.windows,
                },
              ] as const
            ).map((item) => (
              <div className="download-option" key={item.platform}>
                <Icon name={item.icon} size={28} />
                <h3>{item.platform}</h3>
                <p>
                  {item.detail}
                  <br />
                  <span>{item.architecture}</span>
                </p>
                {!release ? (
                  <div className="download-skeleton">
                    <AsciiSkeleton variant="scan" lines={2} />
                    <span className="sr-only">
                      Loading {item.platform} download
                    </span>
                  </div>
                ) : (
                  <a
                    className={`button ${item.platform === "macOS" ? "primary" : ""}`}
                    href={item.url || release.notes}
                  >
                    {item.url
                      ? `Download for ${item.platform}`
                      : "View release options"}
                    <Icon name="download" size={16} />
                  </a>
                )}
              </div>
            ))}
          </div>
        </AsciiReveal>
        <div className="download-links">
          <a href={release?.notes || RELEASES_URL}>
            Release notes <span aria-hidden="true">↗</span>
          </a>
          <a href={`${REPOSITORY}/releases`}>
            All releases <span aria-hidden="true">↗</span>
          </a>
        </div>
        {fallback && (
          <p className="download-notice">
            We couldn’t check for updates. These verified installers are still
            available; see all releases for newer versions.
          </p>
        )}
      </div>
    </section>
  );
}

const questions = [
  [
    "Which meetings can I record?",
    "Choose microphone-only capture for in-person conversations, or mix your microphone with computer audio for online calls. Eavesdrop can detect Zoom, Microsoft Teams, and Google Meet. Always let participants know you’re recording and get any required permission.",
  ],
  [
    "Does my audio leave my computer?",
    "Recording, playback, and Whisper transcription happen locally. Local summaries also run on your device. If you choose a cloud summary provider in the app, the transcript is sent to that provider. Exporting a recording creates a shareable file at the destination you choose.",
  ],
  [
    "Can I use it offline?",
    "Yes. Once you have downloaded the transcription and summary models, local recording, transcription, and summaries can work offline. Model downloads, update checks, and optional cloud summaries need an internet connection.",
  ],
  [
    "What do I need to get started?",
    "A Mac running macOS 13 or later, or a 64-bit Windows 10/11 PC. Grant microphone access when prompted. Capturing computer audio on macOS also requires Screen Recording permission. Install a local model in Settings when you’re ready to transcribe.",
  ],
  [
    "What if an installation triggers a security warning?",
    "Signing status can vary between releases. Check the release notes before installing, and only use downloads from the official Eavesdrop repository. Developer builds may use an ad-hoc signature. The repository documentation explains the macOS signing and notarization setup.",
  ],
];

export default function App() {
  return (
    <>
      <a className="skip-link" href="#main">
        Skip to content
      </a>
      <div className="site-frame">
        <header className="site-header">
          <a className="brand" href="#" aria-label="Eavesdrop home">
            <Icon name="wave" size={25} />
            <span>eavesdrop</span>
          </a>
          <nav aria-label="Main navigation">
            <a href="#how-it-works">How it works</a>
            <a href="#privacy">Privacy</a>
            <a className="header-download" href="#download">
              Download <Icon name="download" size={14} />
            </a>
          </nav>
        </header>
        <main id="main">
          <section className="hero" aria-labelledby="hero-title">
            <div className="hero-copy">
              <p className="hero-platform mono">
                A meeting recorder for macOS & Windows
              </p>
              <AsciiReveal preset="decode" duration="fast">
                <h1 id="hero-title">
                  Stay in the
                  <br />
                  conversation.
                </h1>
              </AsciiReveal>
              <p className="hero-description">
                Let Eavesdrop remember the details.
                <br />
                Record, transcribe, and turn meetings into useful notes — right
                on your computer.
              </p>
              <div className="hero-actions">
                <a className="button primary" href="#download">
                  Get Eavesdrop <Icon name="download" size={17} />
                </a>
                <a className="text-link" href="#preview">
                  Take a closer look <Icon name="arrow" size={17} />
                </a>
              </div>
              <p className="hero-footnote">
                <Icon name="lock" size={13} /> Local recording. On-device
                transcription. Your library.
              </p>
            </div>
            <div className="hero-art" aria-hidden="true">
              <div className="signal-coordinates mono">
                <span>input → memory</span>
                <span>[ e ]</span>
              </div>
              <pre className="ascii-signal">{signal}</pre>
              <div className="signal-caption mono">
                <span>Every word has a place.</span>
                <span>● ─────── 01:24</span>
              </div>
            </div>
          </section>
          <div className="capabilities mono">
            <span>
              <Icon name="wave" size={16} /> Microphone + computer audio
            </span>
            <span>
              <Icon name="file" size={16} /> Transcripts & summaries
            </span>
            <span>
              <Icon name="lock" size={16} /> Encrypted local library
            </span>
          </div>
          <section
            className="preview section"
            id="preview"
            aria-labelledby="preview-title"
          >
            <div className="preview-intro">
              <h2 id="preview-title">
                From “what did we say?”
                <br />
                to right here.
              </h2>
              <p>
                The recording, the words, and the next steps.
                <br />
                Together in a library that stays on your device.
              </p>
            </div>
            <PreviewBoundary>
              <Suspense
                fallback={
                  <div
                    className="preview-fallback"
                    aria-label="Loading product preview"
                    role="status"
                  >
                    <AsciiSkeleton variant="scan" lines={8} />
                  </div>
                }
              >
                <Demo />
              </Suspense>
            </PreviewBoundary>
            <div className="preview-caption mono">
              <span>
                Explore the tabs above. This is an illustrative product preview.
              </span>
              <span>Built to stay out of your way.</span>
            </div>
          </section>
          <section
            className="workflow section"
            id="how-it-works"
            aria-labelledby="workflow-title"
          >
            <div className="section-intro">
              <h2 id="workflow-title">
                Less note-taking.
                <br />
                More being there.
              </h2>
              <p>From the first hello to the follow-up.</p>
            </div>
            <div className="workflow-steps">
              {[
                {
                  number: "01",
                  icon: "mic" as const,
                  title: "Press record. Be present.",
                  text: "Capture your microphone, or include computer audio for online meetings. Eavesdrop lives in your menu bar or system tray, ready when you are.",
                  detail: "In the room. On the call.",
                },
                {
                  number: "02",
                  icon: "wave" as const,
                  title: "Find the words again.",
                  text: "Turn a recording into a timestamped transcript with Whisper, running on your device. Choose the local model that fits your computer.",
                  detail: "Local audio → local transcript",
                },
                {
                  number: "03",
                  icon: "file" as const,
                  title: "Leave with the next steps.",
                  text: "Create a discussion summary, a concise recap, or action items. Use a local model for summaries, and export audio when you want to share it.",
                  detail: "The details. The decisions. The to-dos.",
                },
              ].map((step) => (
                <article className="workflow-step" key={step.number}>
                  <div className="step-top">
                    <span className="mono">{step.number}</span>
                    <Icon name={step.icon} size={22} />
                  </div>
                  <h3>{step.title}</h3>
                  <p>{step.text}</p>
                  <span className="step-detail mono">{step.detail}</span>
                </article>
              ))}
            </div>
          </section>
          <section
            className="privacy section"
            id="privacy"
            aria-labelledby="privacy-title"
          >
            <div className="privacy-illustration" aria-hidden="true">
              <pre>{`       ┌──────────────────┐
       │                  │
       │    ░░▒▒▓▓▒▒░░    │
       │    ░▒▓███▓▒░░    │
       │    ░░▒▓█▓▒░░░    │
       │                  │
       └────────┬─┬───────┘
            ────┴─┴────

       [ your computer ]
               ↑
        your conversations`}</pre>
            </div>
            <div className="privacy-copy">
              <Icon name="lock" size={25} />
              <h2 id="privacy-title">
                A good listener.
                <br />A better secret keeper.
              </h2>
              <p>
                Your conversations deserve a little privacy. Eavesdrop keeps
                recordings in an encrypted local library, with keys protected by
                macOS Keychain or Windows Credential Manager.
              </p>
              <ul className="privacy-list">
                <li>
                  <Icon name="check" size={17} /> Audio encrypted at rest with
                  AES-256-GCM
                </li>
                <li>
                  <Icon name="check" size={17} /> On-device transcription with
                  Whisper
                </li>
                <li>
                  <Icon name="check" size={17} /> Local summaries available
                  after model setup
                </li>
              </ul>
              <p className="privacy-note">
                Prefer cloud summaries? That’s your choice in the app. Your
                transcript is only sent to the provider when you use that
                option.
              </p>
              <a
                className="text-link"
                href={`${REPOSITORY}#storage-and-privacy`}
              >
                Read about storage & privacy <Icon name="arrow" size={16} />
              </a>
            </div>
          </section>
          <Downloads />
          <section className="faq section" aria-labelledby="faq-title">
            <h2 id="faq-title">
              A few things
              <br />
              you might be wondering.
            </h2>
            <div className="faq-list">
              {questions.map(([question, answer]) => (
                <details key={question}>
                  <summary>
                    {question}
                    <span aria-hidden="true">+</span>
                  </summary>
                  <p>{answer}</p>
                </details>
              ))}
            </div>
          </section>
        </main>
        <footer className="site-footer">
          <div>
            <a className="brand" href="#">
              <Icon name="wave" size={23} />
              <span>eavesdrop</span>
            </a>
            <p>A little less typing. A little more listening.</p>
          </div>
          <div className="footer-links">
            <a href={REPOSITORY}>
              <Icon name="github" size={16} /> GitHub
            </a>
            <a href={`${REPOSITORY}/issues`}>Feedback ↗</a>
            <a href="https://quanhoangindex.github.io/ascii-reveal/">
              Made with ascii-reveal ↗
            </a>
          </div>
        </footer>
      </div>
    </>
  );
}
