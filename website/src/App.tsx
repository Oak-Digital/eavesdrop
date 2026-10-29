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
  REPOSITORY,
} from "./releases.mjs";
import snapshot from "./release-snapshot.json";
import { detectPlatform } from "./platform";
import { RecorderHero } from "./RecorderHero";
import { PrivacyVault } from "./PrivacyVault";
import ReleaseNotes from "./ReleaseNotes";

const Demo = lazy(() => import("./Demo"));

function Brand() {
  return (
    <div className="brand-lockup">
      <a className="brand" href="#" aria-label="Eavesdrop home">
        <Icon name="wave" size={25} gradient />
        <span>eavesdrop</span>
      </a>
      <a className="brand-maker" href="https://www.oak.dk/">
        <span>by</span>
        <img src="/oak-logo.svg" alt="" width={18} height={18} />
        <span>Oak</span>
      </a>
    </div>
  );
}

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
  const [detectedPlatform] = useState(detectPlatform);
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
          Ready for your
          <br />
          next meeting?
        </h2>
        <p>
          Download Eavesdrop for Mac or Windows.
          <br />
          Choose your microphone and start recording.
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
            ).slice().sort((a, b) =>
              Number(b.platform === detectedPlatform) - Number(a.platform === detectedPlatform)
            ).map((item) => (
              <div
                className={`download-option${item.platform === detectedPlatform ? " recommended" : ""}`}
                key={item.platform}
              >
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
                    className={`button ${item.platform === detectedPlatform ? "primary" : ""}`}
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
          <a href="#release-notes">Release notes</a>
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
    "Use your microphone for in-person meetings, or include computer audio for online calls. Eavesdrop records directly from your computer without joining as a bot. Let everyone know before you start recording.",
  ],
  [
    "Does my audio leave my computer?",
    "Local processing keeps audio and text on your computer. If you choose Cloud, transcription sends audio to OpenAI, while summaries send text to OpenAI or Anthropic. Cloud processing uses your own API key, and provider charges may apply. You can also export audio to share it yourself.",
  ],
  [
    "Can I use it offline?",
    "Yes. Recording and playback work offline. Download the local AI models in Settings to transcribe and summarize offline too. Model downloads, update checks, and cloud processing need an internet connection.",
  ],
  [
    "What do I need to get started?",
    "A Mac with macOS 13 or later, or a 64-bit Windows 10/11 PC. Allow microphone access; on macOS, computer audio also needs Screen Recording permission. For transcripts and summaries, install local AI models in Settings or connect a cloud provider with your API key.",
  ],
  [
    "What if an installation triggers a security warning?",
    "Check the notes for the version you downloaded. Some development releases may not have the verification that macOS or Windows expects. Use the official GitHub downloads linked here, and check the repository’s installation guidance before proceeding.",
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
          <Brand />
          <nav aria-label="Main navigation">
            <a href="#how-it-works">How it works</a>
            <a href="#privacy">Privacy</a>
            <a href="#release-notes">Release notes</a>
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
                Record in-person meetings and online calls.
                <br />
                Turn them into transcripts, summaries, and action items
                you can come back to.
              </p>
              <div className="hero-actions">
                <a className="button primary" href="#download">
                  Download <Icon name="download" size={17} />
                </a>
                <a className="text-link" href="#preview">
                  See an example <Icon name="arrow" size={17} />
                </a>
              </div>
              <p className="hero-footnote">
                <Icon name="lock" size={13} /> Local processing by default.
                Cloud when you choose.
              </p>
            </div>
            <RecorderHero />
          </section>
          <div className="capabilities mono">
            <span>
              <Icon name="wave" size={16} /> In-person & online meetings
            </span>
            <span>
              <Icon name="file" size={16} /> Transcripts & summaries
            </span>
            <span>
              <Icon name="lock" size={16} /> Audio stored encrypted
            </span>
          </div>
          <section
            className="preview section"
            id="preview"
            aria-labelledby="preview-title"
          >
            <div className="preview-intro">
              <h2 id="preview-title">
                The conversation,
                <br />
                and what comes next.
              </h2>
              <p>
                Your audio, transcript, and summary together.
                <br />
                Listen back, revisit a decision, or check the next steps.
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
                Switch tabs to explore this fictional meeting.
              </span>
              <span>This preview does not record or play audio.</span>
            </div>
          </section>
          <section
            className="workflow section"
            id="how-it-works"
            aria-labelledby="workflow-title"
          >
            <div className="section-intro">
              <h2 id="workflow-title">
                From conversation
                <br />
                to clear notes.
              </h2>
              <p>Record, transcribe, and summarize in one app.</p>
            </div>
            <div className="workflow-steps">
              {[
                {
                  number: "01",
                  icon: "mic" as const,
                  title: "Record your meeting",
                  text: "Capture the room through your microphone, or include computer audio for online calls. Eavesdrop is ready in your menu bar or system tray.",
                  detail: "Saved on your computer.",
                },
                {
                  number: "02",
                  icon: "wave" as const,
                  title: "Turn speech into text",
                  text: "Create a transcript you can read at your own pace. Transcribe on your computer with a local AI model, or use OpenAI in the cloud.",
                  detail: "Local transcripts include timestamps.",
                },
                {
                  number: "03",
                  icon: "file" as const,
                  title: "Create a summary",
                  text: "Turn the transcript into a detailed summary, a quick recap, or action items. Keep the discussion and next steps together, with the recording there for context.",
                  detail: "Automate these steps in Settings.",
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
            <PrivacyVault />
            <div className="privacy-copy">
              <Icon name="lock" size={25} />
              <h2 id="privacy-title">
                Private by default.
                <br />Cloud by choice.
              </h2>
              <p>
                Your recordings are saved on your computer with encrypted audio.
                Transcripts and summaries can stay local too, so you can work
                without uploading your conversations.
              </p>
              <ul className="privacy-list">
                <li>
                  <Icon name="check" size={17} /> Audio encrypted on your computer
                </li>
                <li>
                  <Icon name="check" size={17} /> Local transcripts and summaries
                </li>
                <li>
                  <Icon name="check" size={17} /> Cloud processing is off
                  by default
                </li>
              </ul>
              <p className="privacy-note">
                Choose Cloud and transcription sends audio to OpenAI; summaries
                send text to OpenAI or Anthropic. This also applies to automatic
                processing with Cloud selected.
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
          <ReleaseNotes />
          <section className="faq section" aria-labelledby="faq-title">
            <h2 id="faq-title">
              A few things
              <br />
              to know.
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
            <Brand />
            <p>Less typing. More listening.</p>
          </div>
          <div className="footer-links">
            <a href="#release-notes">Release notes</a>
            <a href={REPOSITORY}>
              <Icon name="github" size={16} /> GitHub
            </a>
            <a href={`${REPOSITORY}/issues`}>Feedback ↗</a>
          </div>
        </footer>
      </div>
    </>
  );
}
