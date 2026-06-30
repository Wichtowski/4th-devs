import { ReactorTaskClient, loadEnv } from "../src/services/index.js";
import { findReactorPath, snapshotReactorState } from "../src/evaluation/reactor.js";

const MAX_TURNS = 50;

const logResponse = (label, response) => {
  const payload = {
    code: response.code,
    message: response.message,
    player: response.player,
    reached_goal: response.reached_goal
  };

  console.log(`${label}: ${JSON.stringify(payload)}`);
};

const main = async () => {
  loadEnv();

  const reactor = ReactorTaskClient.fromEnv();
  let response = await reactor.start();
  logResponse("start", response);

  if (response.code !== 100) {
    return;
  }

  let state = snapshotReactorState(response);

  for (let turn = 0; turn < MAX_TURNS; turn += 1) {
    if (state.player.col === state.goal.col && state.player.row === state.goal.row) {
      console.log("Reactor robot reached the goal");
      return;
    }

    const path = findReactorPath(state);
    if (!path || path.length === 0) {
      throw new Error("No safe reactor path found");
    }

    const command = path[0];
    response = await reactor.command(command);
    logResponse(command, response);

    if (response.code !== 100) {
      return;
    }

    state = snapshotReactorState(response);
  }

  throw new Error("Reactor solver exceeded the maximum number of turns");
};

main().catch((error) => {
  console.error(`Error: ${error.message}`);
  process.exit(1);
});
