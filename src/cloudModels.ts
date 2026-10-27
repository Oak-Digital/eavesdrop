import type { CloudProvider } from "./types";

export const TRANSCRIPTION_SOURCE_OPTIONS = [
  { value: "local", label: "Local", description: "Whisper · Runs on your computer" },
  { value: "cloud", label: "Cloud", description: "OpenAI · Sends audio to the cloud" },
];
export const CLOUD_TRANSCRIPTION_MODEL = "gpt-transcribe";

export const SUMMARY_SOURCE_OPTIONS = [
  { value: "local", label: "Local", description: "Runs on your computer" },
  { value: "cloud", label: "Cloud", description: "Sends the transcript to an API provider" },
];
export const CLOUD_PROVIDERS = [
  { value: "openai", label: "OpenAI" },
  { value: "anthropic", label: "Anthropic" },
];
export const CLOUD_MODELS: Record<CloudProvider, { value: string; label: string }[]> = {
  openai: [
    { value: "gpt-5.4-mini", label: "GPT-5.4 mini" },
    { value: "gpt-5.4", label: "GPT-5.4" },
    { value: "gpt-6.1-sol", label: "GPT-6.1 Sol" },
  ],
  anthropic: [
    { value: "claude-sonnet-5-5", label: "Claude Sonnet 5.5" },
    { value: "claude-haiku-5-5", label: "Claude Haiku 5.5" },
    { value: "claude-opus-5-5", label: "Claude Opus 5.5" },
  ],
};
export const providerName = (provider: CloudProvider) => CLOUD_PROVIDERS.find((item) => item.value === provider)!.label;
