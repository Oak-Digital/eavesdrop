// @vitest-environment jsdom
import { useState } from "react";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { OakOsProjectPicker } from "./OakOsProjectPicker";
import type { OakOsProject } from "./types";

const projects: OakOsProject[] = [
  { id: "z", name: "Website", identifier: "ZE2602", client: { id: "zen", name: "Zénith" } },
  { id: "b", name: "Website", identifier: "AC2602", client: { id: "acme", name: "Acme" } },
  { id: "a", name: "App", identifier: "AC2601", client: { id: "acme", name: "Acme" } },
  { id: "none", name: "Internal" },
];
afterEach(cleanup);
function Harness({ onChoose = () => {} }: { onChoose?: (value: string) => void }) {
  const [value, setValue] = useState("");
  return <><OakOsProjectPicker id="project" projects={projects} value={value} onValueChange={(id) => { setValue(id); onChoose(id); }} />
    <button disabled={!value}>Publish</button></>;
}
function open() {
  const input = screen.getByRole("combobox", { name: "Project" }) as HTMLInputElement;
  fireEvent.focus(input);
  return input;
}

describe("OakOS project picker", () => {
  it("groups and sorts projects by client with unassigned projects last", () => {
    render(<Harness />); open();
    const groups = screen.getAllByRole("group");
    expect(groups.map((group) => group.textContent)).toEqual(["AcmeAppAC2601WebsiteAC2602", "ZénithWebsiteZE2602", "No clientInternal"]);
    expect(within(screen.getByRole("group", { name: "Acme" })).getAllByRole("option")).toHaveLength(2);
    expect((screen.getByRole("button", { name: "Publish" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("searches project names, client names and project codes without case or accent sensitivity", () => {
    render(<Harness />); const input = open();
    fireEvent.change(input, { target: { value: "ZENITH" } });
    expect(screen.getAllByRole("option")).toHaveLength(1);
    expect(screen.getByRole("option").textContent).toBe("WebsiteZE2602");
    fireEvent.change(input, { target: { value: "  acme  web " } });
    expect(screen.getAllByRole("option")).toHaveLength(1);
    expect(screen.getByRole("option").textContent).toBe("WebsiteAC2602");
    fireEvent.change(input, { target: { value: "ac2601" } });
    expect(screen.getByRole("option").textContent).toBe("AppAC2601");
    fireEvent.change(input, { target: { value: "no match" } });
    expect(screen.queryAllByRole("option")).toHaveLength(0);
    expect(screen.getByRole("status").textContent).toBe("No matching projects.");
    fireEvent.change(input, { target: { value: "" } });
    expect(screen.getAllByRole("option")).toHaveLength(4);
  });

  it("selects the correct project ID when names are duplicated and clears stale selection on search", () => {
    const choose = vi.fn(); render(<Harness onChoose={choose} />); const input = open();
    fireEvent.click(within(screen.getByRole("group", { name: "Zénith" })).getByRole("option"));
    expect(choose).toHaveBeenLastCalledWith("z");
    expect(input.value).toBe("Website · Zénith");
    expect(screen.queryByRole("listbox")).toBeNull();
    expect((screen.getByRole("button", { name: "Publish" }) as HTMLButtonElement).disabled).toBe(false);
    fireEvent.click(input);
    fireEvent.change(input, { target: { value: "Acme" } });
    expect(choose).toHaveBeenLastCalledWith("");
    expect((screen.getByRole("button", { name: "Publish" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("supports arrow navigation and Enter, and Escape closes only the picker first", () => {
    const escape = vi.fn(); const choose = vi.fn();
    render(<div onKeyDown={(event) => { if (event.key === "Escape") escape(); }}><Harness onChoose={choose} /></div>);
    const input = open();
    fireEvent.keyDown(input, { key: "ArrowDown" });
    const activeId = input.getAttribute("aria-activedescendant");
    expect(document.getElementById(activeId!)?.textContent).toBe("AppAC2601");
    fireEvent.keyDown(input, { key: "ArrowDown" });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(choose).toHaveBeenLastCalledWith("b");
    expect(input.value).toBe("Website · Acme");
    fireEvent.click(input);
    fireEvent.keyDown(input, { key: "Escape" });
    expect(escape).not.toHaveBeenCalled();
    expect(input.value).toBe("Website · Acme");
    fireEvent.keyDown(input, { key: "Escape" });
    expect(escape).toHaveBeenCalledOnce();
  });

  it("closes on focus leaving and cannot open while publishing", () => {
    const { rerender } = render(<OakOsProjectPicker id="project" projects={projects} value="a" onValueChange={vi.fn()} />);
    const input = open();
    fireEvent.blur(input, { relatedTarget: document.body });
    expect(screen.queryByRole("listbox")).toBeNull();
    rerender(<OakOsProjectPicker id="project" projects={projects} value="a" disabled onValueChange={vi.fn()} />);
    expect(input.disabled).toBe(true);
    fireEvent.focus(input);
    expect(screen.queryByRole("listbox")).toBeNull();
  });
});
