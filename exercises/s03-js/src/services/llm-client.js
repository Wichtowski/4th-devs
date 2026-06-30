import { existsSync } from "node:fs";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { AzureOpenAiWrapper } from "./azure-openai-wrapper.js";
import { JsonSchemaFormat, OpenAiWrapper } from "./openai-wrapper.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const EXERCISES_DIR = path.resolve(__dirname, "../../..");
const AVAILABLE_MODELS_PATH = path.join(EXERCISES_DIR, "available_models");
const LOCAL_ENV_PATH = path.join(EXERCISES_DIR, "s03-js", ".env");
const ROOT_ENV_PATH = path.join(EXERCISES_DIR, ".env");

export const DEFAULT_MODEL = "gpt-5.4-mini";
export const DEFAULT_AZURE_DEPLOYMENT = "gpt-5.4-nano";

let envLoaded = false;

export const loadEnv = () => {
  if (envLoaded) {
    return;
  }

  for (const file of [LOCAL_ENV_PATH, ROOT_ENV_PATH]) {
    if (existsSync(file) && typeof process.loadEnvFile === "function") {
      process.loadEnvFile(file);
    }
  }

  envLoaded = true;
};

export const loadAvailableModels = () => {
  if (!existsSync(AVAILABLE_MODELS_PATH)) {
    return [];
  }

  return readFileSync(AVAILABLE_MODELS_PATH, "utf8")
    .split(/\r?\n/u)
    .map((line) => line.trim())
    .filter(Boolean);
};

export const pickModel = (requested) => {
  const available = loadAvailableModels();
  if (available.length === 0) {
    return requested || DEFAULT_MODEL;
  }

  if (available.includes(requested)) {
    return requested;
  }

  if (available.includes(DEFAULT_MODEL)) {
    return DEFAULT_MODEL;
  }

  return available[0];
};

export const resolveAzureDeployment = (requested) => {
  loadEnv();

  const explicit =
    process.env.ELECTRICITY_AZURE_DEPLOYMENT?.trim()
    || process.env.OPENAI_AZURE_DEPLOYMENT?.trim()
    || "";
  if (explicit) {
    return explicit;
  }

  const available = loadAvailableModels();
  if (available.includes(requested)) {
    return requested;
  }

  if (available.includes(DEFAULT_AZURE_DEPLOYMENT)) {
    return DEFAULT_AZURE_DEPLOYMENT;
  }

  return available[0] ?? DEFAULT_AZURE_DEPLOYMENT;
};

export const resolveProvider = (hasOpenAI, hasAzure) => {
  loadEnv();

  const explicit = process.env.LLM_PROVIDER?.trim().toLowerCase() ?? "";
  if (explicit === "openai" || explicit === "azure") {
    if (explicit === "openai" && !hasOpenAI) {
      throw new Error("LLM_PROVIDER=openai but OPENAI_API_KEY is missing");
    }

    if (explicit === "azure" && !hasAzure) {
      throw new Error(
        "LLM_PROVIDER=azure but Azure is not configured (OPENAI_AZURE_API_KEY + OPENAI_AZURE_ENDPOINT)"
      );
    }

    return explicit;
  }

  if (hasOpenAI && !hasAzure) {
    return "openai";
  }

  if (hasAzure && !hasOpenAI) {
    return "azure";
  }

  return "openai";
};

export const azureEndpointFromEnv = () => {
  loadEnv();

  for (const key of ["OPENAI_AZURE_ENDPOINT", "AZURE_OPENAI_ENDPOINT", "OPENAI_AZURE_BASE_URL"]) {
    const value = process.env[key]?.trim().replace(/\/+$/, "") ?? "";
    if (value) {
      return value;
    }
  }

  return "";
};

export const azureConfigFromEnv = (model) => {
  loadEnv();

  const apiKey = process.env.OPENAI_AZURE_API_KEY?.trim() ?? "";
  if (!apiKey) {
    return null;
  }

  const endpoint = azureEndpointFromEnv();
  if (!endpoint) {
    throw new Error(
      "OPENAI_AZURE_API_KEY is set but endpoint is missing. Set OPENAI_AZURE_ENDPOINT in exercises/.env"
    );
  }

  return {
    apiKey,
    azureEndpoint: endpoint,
    deployment: resolveAzureDeployment(model),
    apiVersion: process.env.OPENAI_AZURE_API_VERSION?.trim() || "2025-03-01-preview"
  };
};

export class LLMClient {
  constructor(provider, openai, azure, model, azureDeployment) {
    this.provider = provider;
    this.openai = openai;
    this.azure = azure;
    this.model = model;
    this.azureDeployment = azureDeployment;
  }

  static fromEnv(requestedModel = "") {
    loadEnv();

    const model = pickModel(
      process.env.ELECTRICITY_MODEL?.trim() || requestedModel || DEFAULT_MODEL
    );

    const openaiKey = process.env.OPENAI_API_KEY?.trim() ?? "";
    const openai = openaiKey ? new OpenAiWrapper(openaiKey) : null;

    const azureConfig = azureConfigFromEnv(model);
    const azure = azureConfig
      ? new AzureOpenAiWrapper(
          azureConfig.apiKey,
          azureConfig.azureEndpoint,
          azureConfig.deployment,
          azureConfig.apiVersion
        )
      : null;

    if (!openai && !azure) {
      throw new Error(
        "Configure OPENAI_API_KEY and/or OPENAI_AZURE_API_KEY with OPENAI_AZURE_ENDPOINT"
      );
    }

    const provider = resolveProvider(Boolean(openai), Boolean(azure));
    const azureDeployment = resolveAzureDeployment(model);

    return new LLMClient(provider, openai, azure, model, azureDeployment);
  }

  async responses(payload) {
    const body = { ...payload };
    body.model = pickModel(String(body.model ?? this.model));

    if (this.provider === "openai") {
      if (!this.openai) {
        throw new Error("OpenAI client not configured");
      }

      return this.openai.responsesRaw(body);
    }

    if (!this.azure) {
      throw new Error("Azure OpenAI client not configured");
    }

    return this.azure.responsesRaw(body);
  }

  async completion(model, systemPrompt, userPrompt, jsonSchemaFormat = null) {
    const useModel = pickModel(model || this.model);

    if (this.provider === "openai") {
      if (!this.openai) {
        throw new Error("OpenAI client not configured");
      }

      return this.openai.completion(useModel, systemPrompt, userPrompt, jsonSchemaFormat);
    }

    if (!this.azure) {
      throw new Error("Azure OpenAI client not configured");
    }

    return this.azure.completion(useModel, systemPrompt, userPrompt, jsonSchemaFormat);
  }
}

export const LlmClient = LLMClient;

export {
  AzureOpenAiWrapper,
  JsonSchemaFormat,
  OpenAiWrapper
};
