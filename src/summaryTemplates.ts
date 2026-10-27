import catalog from "./summaryTemplates.json";

// Shared with both Rust binaries, so prompts and generation budgets stay aligned.
export const SUMMARY_TEMPLATES = catalog.templates;
export const DEFAULT_SUMMARY_TEMPLATE = catalog.defaultId;
export const DEFAULT_CUSTOM_PROMPT = SUMMARY_TEMPLATES.find((item) => item.id === "custom")!.prompt;
export const summaryTemplate = (id: string) => SUMMARY_TEMPLATES.find((item) => item.id === id);
export const SUMMARY_TEMPLATE_OPTIONS = SUMMARY_TEMPLATES.map((item) => ({ value: item.id, label: item.name, description: item.description }));
