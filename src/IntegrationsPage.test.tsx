// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import * as api from "./api";
import "../tests/selectSetup";

beforeEach(api.resetBrowserMock);
afterEach(() => { cleanup(); vi.restoreAllMocks(); });

async function openIntegrations() {
  await api.completeOnboarding({ launchAtLogin: false });
  await api.startRecording({ mode: "online" });
  const recording = await api.stopRecording();
  await api.renameRecording(recording.id, "Planning session");
  render(<App />);
  fireEvent.click(await screen.findByRole("button", { name: "Integrations" }));
  return screen.findByRole("article", { name: "OakOS" });
}

describe("Integration cards", () => {
  it("shows the platform identity and reveals connection setup on request", async () => {
    const card = await openIntegrations();
    expect(within(card).getByRole("img", { name: "OakOS logo" }).getAttribute("src")).toBeTruthy();
    expect(within(card).getByRole("heading", { name: "OakOS" })).toBeTruthy();
    expect(within(card).getByText(/Send recordings to your OakOS projects/)).toBeTruthy();
    expect(screen.queryByLabelText("Personal access token")).toBeNull();
    fireEvent.click(await screen.findByRole("button", { name: "Connect OakOS" }));
    const token = screen.getByLabelText("Personal access token") as HTMLInputElement;
    expect(token.type).toBe("password");
    expect(document.activeElement).toBe(token);
    fireEvent.change(token, { target: { value: "test-token" } });
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    fireEvent.click(screen.getByRole("button", { name: "Connect OakOS" }));
    expect((screen.getByLabelText("Personal access token") as HTMLInputElement).value).toBe("");
  });

  it("shows publishing only after connection and hides it after disconnection", async () => {
    await openIntegrations();
    fireEvent.click(await screen.findByRole("button", { name: "Connect OakOS" }));
    fireEvent.change(screen.getByLabelText("Personal access token"), { target: { value: "test-token" } });
    fireEvent.click(screen.getByRole("button", { name: "Connect" }));
    await screen.findByText("Connected", { exact: true });
    expect(screen.queryByLabelText("Personal access token")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Recordings" }));
    fireEvent.click(await screen.findByRole("button", { name: /Planning session/ }));
    await screen.findByRole("button", { name: "Publish to OakOS" });
    fireEvent.click(screen.getByRole("button", { name: "Integrations" }));
    fireEvent.click(await screen.findByRole("button", { name: "Disconnect" }));
    await screen.findByRole("button", { name: "Connect OakOS" });
    fireEvent.click(screen.getByRole("button", { name: "Recordings" }));
    fireEvent.click(await screen.findByRole("button", { name: /Planning session/ }));
    await waitFor(async () => expect(await api.getOakOsIntegration()).toEqual({ connected: false }));
    expect(screen.queryByRole("button", { name: "Publish to OakOS" })).toBeNull();
  });

  it("keeps setup available after a failed connection", async () => {
    vi.spyOn(api, "connectOakOs").mockRejectedValueOnce(new Error("Invalid access token"));
    await openIntegrations();
    fireEvent.click(await screen.findByRole("button", { name: "Connect OakOS" }));
    fireEvent.change(screen.getByLabelText("Personal access token"), { target: { value: "invalid-token" } });
    fireEvent.click(screen.getByRole("button", { name: "Connect" }));
    expect((await screen.findByRole("alert")).textContent).toBe("Invalid access token");
    expect((await api.getOakOsIntegration()).connected).toBe(false);
    expect((screen.getByRole("button", { name: "Connect" }) as HTMLButtonElement).disabled).toBe(false);
  });
});
