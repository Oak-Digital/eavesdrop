// @vitest-environment jsdom

import { beforeEach, describe, expect, it } from "vitest";
import {
  deleteRecordings,
  listRecordings,
  resetBrowserMock,
  restoreRecordings,
  startRecording,
  stopRecording,
} from "./api";

describe("bulk recording actions", () => {
  beforeEach(resetBrowserMock);

  it("moves and restores multiple recordings together", async () => {
    await startRecording({ mode: "in_person" });
    const first = await stopRecording();
    await startRecording({ mode: "online" });
    const second = await stopRecording();

    await deleteRecordings([first.id, second.id]);
    expect(await listRecordings(false)).toHaveLength(0);
    expect(await listRecordings(true)).toHaveLength(2);

    await restoreRecordings([first.id, second.id]);
    expect(await listRecordings(false)).toHaveLength(2);
    expect(await listRecordings(true)).toHaveLength(0);
  });
});
