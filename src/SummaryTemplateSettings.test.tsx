// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { SummaryTemplateSettings } from "./SummaryTemplateSettings";
import { getSnapshot, resetBrowserMock } from "./api";
import "../tests/selectSetup";

afterEach(() => { cleanup(); resetBrowserMock(); });

async function choose(name: string) {
  fireEvent.keyDown(screen.getByRole("combobox", { name: "Default summary template" }), { key: "Enter" });
  fireEvent.click(await screen.findByRole("option", { name: new RegExp(name) }));
}

describe("default summary template settings", () => {
  it("saves the default independently of the existing custom prompt", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    const settings = { ...(await getSnapshot()).settings, summaryPrompt: "My saved instructions" };
    render(<SummaryTemplateSettings settings={settings} onSave={onSave} />);
    await choose("Concise summary");
    fireEvent.click(screen.getByRole("button", { name: "Save template" }));
    await waitFor(() => expect(onSave).toHaveBeenCalledWith({ summaryTemplate: "concise" }));
    await choose("Custom prompt");
    expect((screen.getByLabelText("Custom summary prompt") as HTMLTextAreaElement).value).toBe("My saved instructions");
    fireEvent.change(screen.getByLabelText("Custom summary prompt"), { target: { value: "Focus on costs." } });
    fireEvent.click(screen.getByRole("button", { name: "Save template" }));
    await waitFor(() => expect(onSave).toHaveBeenLastCalledWith({ summaryTemplate: "custom", summaryPrompt: "Focus on costs." }));
  });

  it("keeps the draft available when saving fails", async () => {
    render(<SummaryTemplateSettings settings={(await getSnapshot()).settings} onSave={vi.fn().mockRejectedValue(new Error("Storage unavailable"))} />);
    await choose("Action items");
    fireEvent.click(screen.getByRole("button", { name: "Save template" }));
    expect((await screen.findByRole("alert")).textContent).toBe("Storage unavailable");
    expect(screen.getByRole("combobox").textContent).toContain("Action items");
    expect((screen.getByRole("button", { name: "Save template" }) as HTMLButtonElement).disabled).toBe(false);
  });
});
