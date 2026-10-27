import { useEffect, useState } from "react";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { parseReleaseHistory, RELEASE_HISTORY_API, REPOSITORY } from "./releases.mjs";
import snapshot from "./release-history-snapshot.json";

const savedReleases = parseReleaseHistory(snapshot);
const dateFormat = new Intl.DateTimeFormat("en-GB", {
  day: "numeric",
  month: "long",
  year: "numeric",
  timeZone: "UTC",
});

export default function ReleaseNotes() {
  const [releases, setReleases] = useState(savedReleases);
  const [status, setStatus] = useState<"loading" | "live" | "fallback">("loading");
  const [expanded, setExpanded] = useState(false);

  useEffect(() => {
    const controller = new AbortController();
    let mounted = true;
    const timeout = window.setTimeout(() => controller.abort(), 6000);
    fetch(RELEASE_HISTORY_API, {
      signal: controller.signal,
      headers: { Accept: "application/vnd.github+json" },
    })
      .then((response) => {
        if (!response.ok) throw new Error("Release history lookup failed");
        return response.json();
      })
      .then((data) => {
        const history = parseReleaseHistory(data);
        if (mounted) {
          setReleases(history);
          setStatus("live");
        }
      })
      .catch(() => {
        if (mounted) setStatus("fallback");
      })
      .finally(() => window.clearTimeout(timeout));
    return () => {
      mounted = false;
      window.clearTimeout(timeout);
      controller.abort();
    };
  }, []);

  return (
    <section className="release-notes section" id="release-notes" aria-labelledby="release-notes-title">
      <div className="section-intro">
        <h2 id="release-notes-title">Release notes</h2>
        <p>What’s new, improved, and fixed in Eavesdrop.</p>
      </div>
      <p className="release-history-status" role="status">
        {status === "loading"
          ? "Showing saved release notes. Checking for updates…"
          : status === "fallback"
            ? "Showing saved release notes. We couldn’t check for updates; see GitHub for newer releases."
            : "Recent releases, newest first."}
      </p>
      <div id="release-history">
        {(expanded ? releases : releases.slice(0, 3)).map((release) => (
          <article className="release-entry" key={release.version}>
            <header className="release-meta">
              <h3>{release.version}</h3>
              <time dateTime={release.publishedAt}>{dateFormat.format(new Date(release.publishedAt))}</time>
              <a href={release.notes}>View on GitHub <span aria-hidden="true">↗</span></a>
            </header>
            <div className="release-body">
              {release.body ? (
                <Markdown
                  remarkPlugins={[remarkGfm]}
                  skipHtml
                  components={{
                    h1: ({ children }) => <h4>{children}</h4>,
                    h2: ({ children }) => <h4>{children}</h4>,
                    h3: ({ children }) => <h4>{children}</h4>,
                    img: ({ alt }) => <span>{alt}</span>,
                  }}
                >
                  {release.body}
                </Markdown>
              ) : (
                <p>No written notes for this release. <a href={release.notes}>View the release on GitHub.</a></p>
              )}
            </div>
          </article>
        ))}
      </div>
      <div className="release-history-actions">
        {releases.length > 3 && (
          <button className="button" aria-expanded={expanded} aria-controls="release-history" onClick={() => setExpanded(!expanded)}>
            {expanded ? "Show fewer releases" : "Show older releases"}
          </button>
        )}
        <a className="text-link" href={`${REPOSITORY}/releases`}>All releases on GitHub <span aria-hidden="true">↗</span></a>
      </div>
    </section>
  );
}
