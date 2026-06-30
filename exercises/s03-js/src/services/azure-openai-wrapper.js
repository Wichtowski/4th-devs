import { extractResponseText } from "./openai-wrapper.js";

const DEFAULT_API_VERSION = "2025-03-01-preview";

const buildAzureResponsesUrl = (azureEndpoint, deployment, apiVersion) => {
  const normalizedEndpoint = azureEndpoint.replace(/\/+$/, "");
  const encodedDeployment = encodeURIComponent(deployment);
  const encodedApiVersion = encodeURIComponent(apiVersion);
  return `${normalizedEndpoint}/openai/deployments/${encodedDeployment}/responses?api-version=${encodedApiVersion}`;
};

const buildJsonSchemaFormat = (jsonSchemaFormat) => ({
  format: {
    type: "json_schema",
    name: jsonSchemaFormat.name,
    strict: jsonSchemaFormat.strict,
    schema: jsonSchemaFormat.schema
  }
});

const postJson = async (url, payload, headers) => {
  const response = await fetch(url, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      ...headers
    },
    body: JSON.stringify(payload)
  });

  const text = await response.text();
  let data;
  try {
    data = JSON.parse(text);
  } catch {
    throw new Error(`Response was not valid JSON: ${text}`);
  }

  const errorMessage =
    data && typeof data === "object" && !Array.isArray(data) && data.error?.message
      ? data.error.message
      : null;

  if (!response.ok) {
    throw new Error(`Request failed (${response.status}): ${errorMessage ?? "Unknown API error"}`);
  }

  if (errorMessage) {
    throw new Error(errorMessage);
  }

  return data;
};

export class AzureOpenAiWrapper {
  constructor(apiKey, azureEndpoint, deployment, apiVersion = DEFAULT_API_VERSION) {
    this.apiKey = apiKey;
    this.azureEndpoint = azureEndpoint;
    this.deployment = deployment;
    this.apiVersion = apiVersion;
  }

  static fromEnv(deployment = null) {
    const apiKey = process.env.OPENAI_AZURE_API_KEY?.trim() ?? "";
    const azureEndpoint = process.env.OPENAI_AZURE_ENDPOINT?.trim() ?? "";
    const apiVersion = process.env.OPENAI_AZURE_API_VERSION?.trim() ?? DEFAULT_API_VERSION;
    const deploy =
      deployment
      ?? process.env.ELECTRICITY_AZURE_DEPLOYMENT?.trim()
      ?? process.env.OPENAI_AZURE_DEPLOYMENT?.trim()
      ?? "";

    if (!apiKey) {
      throw new Error("Missing OPENAI_AZURE_API_KEY in environment");
    }

    if (!azureEndpoint) {
      throw new Error("Missing OPENAI_AZURE_ENDPOINT in environment");
    }

    if (!deploy) {
      throw new Error(
        "Missing Azure deployment name (ELECTRICITY_AZURE_DEPLOYMENT or OPENAI_AZURE_DEPLOYMENT)"
      );
    }

    return new AzureOpenAiWrapper(apiKey, azureEndpoint, deploy, apiVersion);
  }

  async responsesRaw(payload) {
    const body = { ...payload };
    delete body.model;

    const url = buildAzureResponsesUrl(this.azureEndpoint, this.deployment, this.apiVersion);
    return postJson(url, body, {
      "api-key": this.apiKey
    });
  }

  async completion(model, systemPrompt, userPrompt, jsonSchemaFormat = null) {
    void model;
    const payload = this.#buildPayload(systemPrompt, userPrompt, jsonSchemaFormat);
    const parsed = await this.responsesRaw(payload);
    const text = extractResponseText(parsed);

    if (!text) {
      throw new Error(`No text found in Azure response: ${JSON.stringify(parsed)}`);
    }

    return text;
  }

  #buildPayload(systemPrompt, userPrompt, jsonSchemaFormat = null) {
    const payload = {
      input: [
        {
          role: "system",
          content: [{ type: "input_text", text: systemPrompt }]
        },
        {
          role: "user",
          content: [{ type: "input_text", text: userPrompt }]
        }
      ]
    };

    if (jsonSchemaFormat) {
      payload.text = buildJsonSchemaFormat(jsonSchemaFormat);
    }

    return payload;
  }
}

export { DEFAULT_API_VERSION };
