// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import * as api from "./api";
import { CloudModelSettings } from "./CloudModelSettings";
import "../tests/selectSetup";

afterEach(() => { cleanup(); vi.restoreAllMocks(); api.resetBrowserMock(); });
async function choose(label: string, name: RegExp) {
  fireEvent.keyDown(screen.getByRole("combobox", { name: label }), { key: "Enter" });
  fireEvent.click(await screen.findByRole("option", { name }));
}

describe("Cloud model settings", () => {
  it("keeps credentials separate from settings and clears them after saving", async () => {
    const saveKey = vi.spyOn(api, "saveCloudApiKey").mockResolvedValue(undefined);
    const onSave = vi.fn(); const onKeysChanged = vi.fn().mockResolvedValue(undefined);
    render(<CloudModelSettings settings={(await api.getSnapshot()).settings} keys={[]} keyError={null} onSave={onSave} onKeysChanged={onKeysChanged} />);
    expect((screen.getByRole("button", { name: "Use Cloud model" }) as HTMLButtonElement).disabled).toBe(true);
    const key = screen.getByLabelText("OpenAI API key") as HTMLInputElement;
    expect(key.type).toBe("password");
    fireEvent.change(key, { target: { value: "dummy-key" } });
    fireEvent.click(screen.getByRole("button", { name: "Save key" }));
    await waitFor(() => expect(onKeysChanged).toHaveBeenCalledOnce());
    expect(saveKey).toHaveBeenCalledWith("openai", "dummy-key");
    expect(key.value).toBe(""); expect(onSave).not.toHaveBeenCalled();
  });

  it("supports provider-specific keys, custom models, removal, and a cloud default", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined); const onKeysChanged = vi.fn().mockResolvedValue(undefined);
    const remove = vi.spyOn(api, "removeCloudApiKey").mockResolvedValue(undefined);
    render(<CloudModelSettings settings={(await api.getSnapshot()).settings} keys={[{ provider: "anthropic", keyStored: true }]} keyError={null} onSave={onSave} onKeysChanged={onKeysChanged} />);
    fireEvent.change(screen.getByLabelText("OpenAI API key"), { target: { value: "unused-draft" } });
    await choose("Cloud provider", /Anthropic/);
    expect((screen.getByLabelText("Anthropic API key") as HTMLInputElement).value).toBe("");
    expect(screen.getByRole("combobox", { name: "Cloud model" }).textContent).toContain("Claude Sonnet");
    await choose("Cloud model", /Custom model ID/);
    expect((screen.getByRole("button", { name: "Use Cloud model" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.change(screen.getByLabelText("Custom model ID"), { target: { value: "claude-custom" } });
    fireEvent.click(screen.getByRole("button", { name: "Use Cloud model" }));
    await waitFor(() => expect(onSave).toHaveBeenCalledWith({ summarySource: "cloud", cloudProvider: "anthropic", cloudModel: "claude-custom" }));
    await waitFor(() => expect((screen.getByRole("button", { name: "Remove key" }) as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(screen.getByRole("button", { name: "Remove key" }));
    await waitFor(() => expect(remove).toHaveBeenCalledWith("anthropic"));
  });

  it("shows credential-store errors without claiming the key was saved", async () => {
    vi.spyOn(api, "saveCloudApiKey").mockRejectedValue(new Error("Credential store unavailable"));
    render(<CloudModelSettings settings={(await api.getSnapshot()).settings} keys={[]} keyError={null} onSave={vi.fn()} onKeysChanged={vi.fn()} />);
    fireEvent.change(screen.getByLabelText("OpenAI API key"), { target: { value: "dummy-key" } });
    fireEvent.click(screen.getByRole("button", { name: "Save key" }));
    expect((await screen.findByRole("alert")).textContent).toBe("Credential store unavailable");
    expect(screen.queryByText(/API key saved in/)).toBeNull();
  });
});
