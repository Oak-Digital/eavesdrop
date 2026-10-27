// @vitest-environment jsdom
import { useState } from "react";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Select } from "./Select";
import "../tests/selectSetup";

const options = [
  { value: "", label: "System default" },
  { value: "unavailable", label: "Unavailable", disabled: true },
  { value: "external", label: "External microphone", description: "USB audio" },
];
afterEach(cleanup);

function ControlledSelect() {
  const [value, setValue] = useState("");
  return <Select label="Microphone" value={value} onValueChange={setValue} options={options} />;
}

describe("Select", () => {
  it("selects options and can return to an empty application value", async () => {
    render(<ControlledSelect />);
    const trigger = screen.getByRole("combobox", { name: "Microphone" });
    fireEvent.keyDown(trigger, { key: "Enter" });
    fireEvent.click(await screen.findByRole("option", { name: /External microphone/ }));
    await waitFor(() => expect(trigger.textContent).toContain("External microphone"));
    fireEvent.keyDown(trigger, { key: "Enter" });
    fireEvent.click(await screen.findByRole("option", { name: "System default" }));
    await waitFor(() => expect(trigger.textContent).toBe("System default"));
  });

  it("supports keyboard selection while skipping disabled options", async () => {
    render(<ControlledSelect />);
    const trigger = screen.getByRole("combobox", { name: "Microphone" });
    fireEvent.keyDown(trigger, { key: "ArrowDown" });
    const first = await screen.findByRole("option", { name: "System default" });
    await waitFor(() => expect(document.activeElement).toBe(first));
    fireEvent.keyDown(first, { key: "ArrowDown" });
    const next = screen.getByRole("option", { name: /External microphone/ });
    await waitFor(() => expect(document.activeElement).toBe(next));
    fireEvent.keyDown(next, { key: "Enter" });
    await waitFor(() => expect(trigger.textContent).toContain("External microphone"));
    await waitFor(() => expect(document.activeElement).toBe(trigger));
  });

  it("dismisses with Escape without selecting or closing a parent dialog", async () => {
    const parentEscape = vi.fn();
    const onValueChange = vi.fn();
    render(<div onKeyDown={parentEscape}><Select label="Project" value="" options={options.slice(2)} onValueChange={onValueChange} placeholder="Choose a project…" /></div>);
    const trigger = screen.getByRole("combobox", { name: "Project" });
    fireEvent.keyDown(trigger, { key: "Enter" });
    parentEscape.mockClear();
    fireEvent.keyDown(await screen.findByRole("option"), { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("listbox")).toBeNull());
    expect(onValueChange).not.toHaveBeenCalled();
    expect(parentEscape).not.toHaveBeenCalled();
    expect(trigger.textContent).toBe("Choose a project…");
    await waitFor(() => expect(document.activeElement).toBe(trigger));
  });

  it("disables empty and explicitly disabled controls", () => {
    const { rerender } = render(<Select label="Model" value="" options={[]} onValueChange={vi.fn()} />);
    expect((screen.getByRole("combobox") as HTMLButtonElement).disabled).toBe(true);
    rerender(<Select label="Model" value="" options={options} disabled onValueChange={vi.fn()} />);
    expect((screen.getByRole("combobox") as HTMLButtonElement).disabled).toBe(true);
  });
});
