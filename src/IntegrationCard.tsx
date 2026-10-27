import type { ReactNode } from "react";

export function IntegrationCard({ id, title, description, logo, connected, children }: {
  id: string;
  title: string;
  description: string;
  logo: string;
  connected: boolean | null;
  children: ReactNode;
}) {
  return (
    <article className="integration-card" aria-labelledby={`${id}-title`}>
      <div className="integration-card-heading">
        <img className="integration-logo" src={logo} alt={`${title} logo`} width={40} height={40} />
        <h2 id={`${id}-title`}>{title}</h2>
        <span className={connected ? "integration-status connected" : "integration-status"} role="status">
          {connected === null ? "Checking…" : connected ? "Connected" : "Not connected"}
        </span>
      </div>
      <p className="integration-description">{description}</p>
      <div className="integration-card-controls">{children}</div>
    </article>
  );
}
