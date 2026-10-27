// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import * as api from "./api";

afterEach(api.resetBrowserMock);

describe("Cloud browser preview contract", () => {
  it("isolates provider keys from each other and unrelated integrations", async () => {
    await api.saveCloudApiKey("openai", "dummy-test-key");
    await api.disconnectOakOs();
    expect(await api.getCloudKeyStatuses()).toEqual([{ provider: "openai", keyStored: true }, { provider: "anthropic", keyStored: false }]);
    expect(JSON.stringify(await api.getSnapshot())).not.toContain("dummy-test-key");
    await api.removeCloudApiKey("anthropic");
    expect((await api.getCloudKeyStatuses())[0].keyStored).toBe(true);
    await api.removeCloudApiKey("openai");
    expect((await api.getCloudKeyStatuses())[0].keyStored).toBe(false);
  });

  it("does not fall back or replace a summary after a missing Cloud key", async () => {
    await api.startRecording({ mode: "online" });
    const recording = await api.stopRecording();
    const local = await api.summarizeRecording(recording.id, "discussion", { source: "local" });
    await api.updateSettings({ summarySource: "cloud" });
    await expect(api.summarizeRecording(recording.id)).rejects.toThrow("API key");
    expect((await api.listRecordings())[0].summary).toEqual(local.summary);
    await api.saveCloudApiKey("openai", "dummy-test-key");
    const cloud = await api.summarizeRecording(recording.id);
    expect(cloud.summary?.model).toContain("Cloud · OpenAI");
    expect(cloud.summary?.templateId).toBe("discussion");
    await api.installSummaryModel("qwen2.5-1.5b");
    expect((await api.getSnapshot()).settings.summarySource).toBe("local");
  });
});
