const VERIFY_URL = "https://hub.ag3nts.org/verify";

export class VerificationError extends Error {
  constructor(statusCode, body) {
    const code = typeof body?.code === "number" ? body.code : -1;
    const message = typeof body?.message === "string" ? body.message : "Unknown error";
    super(`[${statusCode}] (${code}) ${message}`);
    this.name = "VerificationError";
    this.statusCode = statusCode;
    this.body = body;
    this.code = code;
    this.messageText = message;
    this.debug = body?.debug ?? {};
  }
}

export class AiDevsVerification {
  constructor(apiKey) {
    this.apiKey = apiKey;
  }

  static fromEnv() {
    const apiKey = process.env.DEVS_KEY?.trim() ?? "";
    if (!apiKey) {
      throw new Error("Missing or empty DEVS_KEY in environment");
    }

    return new AiDevsVerification(apiKey);
  }

  async verify(task, answer) {
    if (typeof task !== "string" || !task.trim()) {
      throw new Error("Task cannot be empty");
    }

    const response = await fetch(VERIFY_URL, {
      method: "POST",
      headers: {
        "Content-Type": "application/json"
      },
      body: JSON.stringify({
        apikey: this.apiKey,
        task,
        answer
      })
    });

    let body;
    try {
      body = await response.json();
    } catch {
      body = { message: await response.text() };
    }

    if (!response.ok) {
      throw new VerificationError(response.status, body);
    }

    return body;
  }
}
