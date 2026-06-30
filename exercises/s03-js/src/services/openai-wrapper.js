const OPENAI_RESPONSES_URL = "https://api.openai.com/v1/responses";

export class JsonSchemaFormat {
  constructor(name, schema, strict = true) {
    this.name = name;
    this.schema = schema;
    this.strict = strict;
  }
}

const buildJsonSchemaFormat = (jsonSchemaFormat) => ({
  format: {
    type: "json_schema",
    name: jsonSchemaFormat.name,
    strict: jsonSchemaFormat.strict,
    schema: jsonSchemaFormat.schema
  }
});

const extractErrorMessage = (value) => {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }

  const error = value.error;
  if (error && typeof error === "object" && !Array.isArray(error) && typeof error.message === "string") {
    return error.message;
  }

  return null;
};

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

  const errorMessage = extractErrorMessage(data);
  if (!response.ok) {
    throw new Error(`Request failed (${response.status}): ${errorMessage ?? "Unknown API error"}`);
  }

  if (errorMessage) {
    throw new Error(errorMessage);
  }

  return data;
};

export const extractToolCalls = (response) => {
  const output = response?.output;
  if (!Array.isArray(output)) {
    return [];
  }

  return output.filter((item) => item && typeof item === "object" && item.type === "function_call");
};

export const extractResponseText = (response) => {
  const outputText = response?.output_text;
  if (typeof outputText === "string" && outputText.trim()) {
    return outputText.trim();
  }

  const output = response?.output;
  if (!Array.isArray(output)) {
    return null;
  }

  for (const item of output) {
    if (!item || typeof item !== "object" || item.type !== "message") {
      continue;
    }

    const contents = item.content;
    if (!Array.isArray(contents)) {
      continue;
    }

    for (const content of contents) {
      const text = content?.text;
      if (typeof text === "string" && text.trim()) {
        return text.trim();
      }
    }
  }

  return null;
};

export class OpenAiWrapper {
  constructor(apiKey, endpoint = OPENAI_RESPONSES_URL, extraHeaders = {}) {
    this.apiKey = apiKey;
    this.endpoint = endpoint;
    this.extraHeaders = extraHeaders;
  }

  static fromEnv() {
    const apiKey = process.env.OPENAI_API_KEY?.trim() ?? "";
    if (!apiKey) {
      throw new Error("Missing or empty OPENAI_API_KEY in environment");
    }

    return new OpenAiWrapper(apiKey);
  }

  async completion(model, systemPrompt, userPrompt, jsonSchemaFormat = null) {
    const payload = this.#buildPayload(model, systemPrompt, userPrompt, jsonSchemaFormat);
    const parsed = await this.responsesRaw(payload);
    const text = extractResponseText(parsed);

    if (!text) {
      throw new Error(`No text found in response: ${JSON.stringify(parsed)}`);
    }

    return text;
  }

  async responsesRaw(payload) {
    return postJson(this.endpoint, payload, {
      Authorization: `Bearer ${this.apiKey}`,
      ...this.extraHeaders
    });
  }

  #buildPayload(model, systemPrompt, userPrompt, jsonSchemaFormat = null) {
    const payload = {
      model,
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
