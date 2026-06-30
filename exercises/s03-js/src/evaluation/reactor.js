const BOARD_WIDTH = 7;
const BLOCK_MIN_TOP_ROW = 1;
const BLOCK_MAX_TOP_ROW = 4;

const COMMANDS = ["right", "wait", "left"];
const DIRECTION_STEP = {
  up: -1,
  down: 1
};

const commandDelta = (command) => {
  if (command === "left") {
    return -1;
  }

  if (command === "right") {
    return 1;
  }

  return 0;
};

const normalizeBlock = (block) => ({
  col: block.col,
  top_row: block.top_row,
  bottom_row: block.bottom_row ?? block.top_row + 1,
  direction: block.direction
});

export const snapshotReactorState = (response) => {
  if (!response || typeof response !== "object") {
    throw new Error("Reactor response is empty");
  }

  if (!response.player || !response.goal || !Array.isArray(response.blocks)) {
    throw new Error("Reactor response does not include a full board state");
  }

  return {
    player: {
      col: response.player.col,
      row: response.player.row
    },
    goal: {
      col: response.goal.col,
      row: response.goal.row
    },
    blocks: response.blocks.map(normalizeBlock)
  };
};

export const advanceBlock = (block) => {
  const step = DIRECTION_STEP[block.direction];
  if (typeof step !== "number") {
    throw new Error(`Unknown reactor block direction: ${block.direction}`);
  }

  let topRow = block.top_row + step;
  let direction = block.direction;

  if (topRow <= BLOCK_MIN_TOP_ROW) {
    topRow = BLOCK_MIN_TOP_ROW;
    direction = "down";
  } else if (topRow >= BLOCK_MAX_TOP_ROW) {
    topRow = BLOCK_MAX_TOP_ROW;
    direction = "up";
  }

  return {
    col: block.col,
    top_row: topRow,
    bottom_row: topRow + 1,
    direction
  };
};

export const advanceReactorState = (state, command) => {
  const blocks = state.blocks.map(advanceBlock);
  const delta = commandDelta(command);
  const playerCol = Math.min(BOARD_WIDTH, Math.max(1, state.player.col + delta));
  const player = {
    col: playerCol,
    row: state.player.row
  };

  const crushed = blocks.some(
    (block) => block.col === player.col && block.top_row === 4 && player.row === 5
  );

  return {
    player,
    goal: state.goal,
    blocks,
    reached_goal: player.col === state.goal.col && player.row === state.goal.row,
    crushed
  };
};

const stateKey = (state) => {
  const blocks = [...state.blocks]
    .sort((left, right) => left.col - right.col)
    .map((block) => `${block.col}:${block.top_row}:${block.direction}`)
    .join("|");

  return `${state.player.col}:${state.player.row}|${blocks}`;
};

export const findReactorPath = (initialState, maxDepth = 28) => {
  const queue = [{ state: initialState, path: [] }];
  const seen = new Set([stateKey(initialState)]);

  while (queue.length > 0) {
    const { state, path } = queue.shift();

    if (state.player.col === state.goal.col && state.player.row === state.goal.row) {
      return path;
    }

    if (path.length >= maxDepth) {
      continue;
    }

    for (const command of COMMANDS) {
      const next = advanceReactorState(state, command);
      if (next.crushed) {
        continue;
      }

      const key = stateKey(next);
      if (seen.has(key)) {
        continue;
      }

      seen.add(key);
      queue.push({
        state: next,
        path: [...path, command]
      });
    }
  }

  return null;
};
