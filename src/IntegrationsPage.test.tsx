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
  it("opens browser sign-in without asking for an API key", async () => {
    const card = await openIntegrations();
    const connect = vi.spyOn(api, "connectOakOs");
    expect(within(card).getByRole("img", { name: "OakOS logo" }).getAttribute("src")).toBeTruthy();
    expect(within(card).getByRole("heading", { name: "OakOS" })).toBeTruthy();
    expect(within(card).getByText(/Send recordings to your OakOS projects/)).toBeTruthy();
    expect(screen.queryByLabelText("Personal access token")).toBeNull();
    fireEvent.click(await screen.findByRole("button", { name: "Connect OakOS" }));
    await screen.findByText("Connected", { exact: true });
    expect(connect).toHaveBeenCalledWith();
  });

  it("waits for approval and lets the user cancel and retry", async () => {
    let rejectLogin!: (error: Error) => void;
    vi.spyOn(api, "connectOakOs").mockImplementationOnce(() => new Promise((_, reject) => { rejectLogin = reject; }));
    const cancel = vi.spyOn(api, "cancelOakOsLogin").mockImplementationOnce(async () => { rejectLogin(new Error("Sign-in canceled")); });
    await openIntegrations();
    fireEvent.click(await screen.findByRole("button", { name: "Connect OakOS" }));
    expect((screen.getByRole("button", { name: "Waiting for sign-in…" }) as HTMLButtonElement).disabled).toBe(true);
    expect((await api.getOakOsIntegration()).connected).toBe(false);
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    await screen.findByText("Sign-in canceled");
    expect(cancel).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: "Connect OakOS" }));
    await screen.findByText("Connected", { exact: true });
  });

  it("cancels pending sign-in when leaving Integrations", async () => {
    let rejectLogin!: (error: Error) => void;
    vi.spyOn(api, "connectOakOs").mockImplementationOnce(() => new Promise((_, reject) => { rejectLogin = reject; }));
    const cancel = vi.spyOn(api, "cancelOakOsLogin").mockImplementationOnce(async () => { rejectLogin(new Error("Sign-in canceled")); });
    await openIntegrations();
    fireEvent.click(await screen.findByRole("button", { name: "Connect OakOS" }));
    fireEvent.click(screen.getByRole("button", { name: "Recordings" }));
    await waitFor(() => expect(cancel).toHaveBeenCalledOnce());
    fireEvent.click(screen.getByRole("button", { name: "Integrations" }));
    expect((await screen.findByRole("button", { name: "Connect OakOS" }) as HTMLButtonElement).disabled).toBe(false);
  });

  it("shows publishing only after connection and hides it after disconnection", async () => {
    await openIntegrations();
    fireEvent.click(await screen.findByRole("button", { name: "Connect OakOS" }));
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
    vi.spyOn(api, "connectOakOs").mockRejectedValueOnce(new Error("Sign-in was declined"));
    await openIntegrations();
    fireEvent.click(await screen.findByRole("button", { name: "Connect OakOS" }));
    expect((await screen.findByRole("alert")).textContent).toBe("Sign-in was declined");
    expect((await api.getOakOsIntegration()).connected).toBe(false);
    expect((screen.getByRole("button", { name: "Connect OakOS" }) as HTMLButtonElement).disabled).toBe(false);
  });
});
