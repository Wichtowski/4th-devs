const SHELL_URL = "https://hub.ag3nts.org/api/shell";

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

const readJsonBody = async (response) => {
  const text = await response.text();
  try {
    return JSON.parse(text);
  } catch {
    return { code: response.status, message: text };
  }
};

export class ShellError extends Error {
  constructor(statusCode, body) {
    const message = typeof body?.message === "string" ? body.message : "Unknown shell error";
    super(`[${statusCode}] ${message}`);
    this.name = "ShellError";
    this.statusCode = statusCode;
    this.body = body;
    this.code = body?.code ?? statusCode;
    this.data = body?.data;
  }
}

export class HubShellClient {
  constructor(apiKey, { retries = 3, timeoutMs = 60_000 } = {}) {
    this.apiKey = apiKey;
    this.retries = retries;
    this.timeoutMs = timeoutMs;
  }

  static fromEnv(options = {}) {
    const apiKey = process.env.DEVS_KEY?.trim() ?? "";
    if (!apiKey) {
      throw new Error("Missing or empty DEVS_KEY in environment");
    }

    return new HubShellClient(apiKey, options);
  }

  async command(cmd) {
    if (typeof cmd !== "string" || !cmd.trim()) {
      throw new Error("Shell command cannot be empty");
    }

    let lastError = null;
    for (let attempt = 0; attempt <= this.retries; attempt += 1) {
      try {
        const response = await this.#post(cmd);
        const body = await readJsonBody(response);

        if (!response.ok) {
          throw new ShellError(response.status, body);
        }

        return body;
      } catch (error) {
        lastError = error;

        const statusCode = error instanceof ShellError ? error.statusCode : 0;
        if (statusCode !== 429 && statusCode !== 503) {
          throw error;
        }

        if (attempt === this.retries) {
          throw error;
        }

        await sleep(1000 * (attempt + 1));
      }
    }

    throw lastError ?? new Error("Shell command failed");
  }

  async help() {
    return this.command("help");
  }

  async ls(path = "") {
    return this.command(path ? `ls ${path}` : "ls");
  }

  async cat(path) {
    return this.command(`cat ${path}`);
  }

  async rm(path) {
    return this.command(`rm ${path}`);
  }

  async editline(path, lineNumber, content) {
    return this.command(`editline ${path} ${lineNumber} ${content}`);
  }

  async cd(path = "") {
    return this.command(path ? `cd ${path}` : "cd");
  }

  async find(pattern) {
    return this.command(`find ${pattern}`);
  }

  async reboot() {
    return this.command("reboot");
  }

  async pwd() {
    return this.command("pwd");
  }

  async whoami() {
    return this.command("whoami");
  }

  async history() {
    return this.command("history");
  }

  async #post(cmd) {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), this.timeoutMs);

    try {
      return await fetch(SHELL_URL, {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          apikey: this.apiKey,
          cmd
        }),
        signal: controller.signal
      });
    } finally {
      clearTimeout(timeout);
    }
  }
}
