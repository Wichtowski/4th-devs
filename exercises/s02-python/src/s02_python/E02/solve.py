import json
from pathlib import Path

from s02_python.E02.board import ElectricityBoard, should_reset_on_start
from s02_python.E02.tiles import format_board_for_model, rotation_steps
from s02_python.services import AiDevsVerification

_DATA_DIR = Path(__file__).resolve().parent


def load_json(name: str) -> list[list[int]]:
    raw = (_DATA_DIR / name).read_text(encoding="utf-8")
    return json.loads(raw)


def cell_id(row: int, col: int) -> str:
    return f"{row + 1}x{col + 1}"


def plan_rotations(current: list[list[int]], target: list[list[int]]) -> list[str]:
    steps: list[str] = []
    for row in range(3):
        for col in range(3):
            from_id = current[row][col]
            to_id = target[row][col]
            count = rotation_steps(from_id, to_id)
            if count < 0:
                raise ValueError(
                    f"No rotation path at {cell_id(row, col)}: {from_id} -> {to_id}"
                )
            steps.extend(cell_id(row, col) for _ in range(count))
    return steps


def run() -> None:
    verification = AiDevsVerification.from_env()
    board = ElectricityBoard(verification)
    target = load_json("electricity-target.json")

    print(f"Snapshots: {board.snapshots_dir}")
    print(f"Live board PNG: {board.png_url}")
    print(f"Live board JSON: {board.json_url}\n")

    if should_reset_on_start():
        board.reset()
        print("[0] Reset and saved initial snapshot")
    else:
        board.sync_from_hub()
        print("[0] Synced current hub state")

    current = board.fetch_json()
    print("\nCurrent board:")
    print(format_board_for_model(current))
    print("\nTarget board:")
    print(format_board_for_model(target))

    steps = plan_rotations(current, target)
    print(f"\nPlanned {len(steps)} rotation(s): {', '.join(steps)}")

    for index, rotate in enumerate(steps, 1):
        result = board.rotate(rotate)
        message = str(result.get("message", ""))
        print(f"[{index}/{len(steps)}] {rotate} -> {message}")
        if "FLG:" in message:
            print(f"\nFlag: {message}")
            return

    final = board.fetch_json()
    print(f"\nFinal JSON: {json.dumps(final)}")
    print(format_board_for_model(final))


if __name__ == "__main__":
    run()
