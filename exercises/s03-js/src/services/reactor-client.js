import { AiDevsVerification } from "./aidevs-verification.js";

const TASK = "reactor";
const COMMANDS = new Set(["start", "reset", "wait", "left", "right"]);

export class ReactorTaskClient {
  constructor(verification) {
    this.verification = verification;
  }

  static fromEnv() {
    return new ReactorTaskClient(AiDevsVerification.fromEnv());
  }

  async command(command) {
    if (!COMMANDS.has(command)) {
      throw new Error(`Unsupported reactor command: ${command}`);
    }

    return this.verification.verify(TASK, { command });
  }

  start() {
    return this.command("start");
  }

  reset() {
    return this.command("reset");
  }

  wait() {
    return this.command("wait");
  }

  left() {
    return this.command("left");
  }

  right() {
    return this.command("right");
  }
}
