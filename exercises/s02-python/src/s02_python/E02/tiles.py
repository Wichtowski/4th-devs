N = 1
E = 2
S = 4
W = 8

ROTATE_CW: dict[int, int] = {
    0: 3,
    1: 4,
    2: 5,
    3: 6,
    4: 1,
    5: 8,
    6: 7,
    7: 0,
    8: 9,
    9: 2,
}

MASK_BY_ID: dict[int, int] = {
    0: N | E,
    1: N | S,
    2: W | E | N,
    3: S | E,
    4: W | E,
    5: W | E | S,
    6: W | S,
    7: W | N,
    8: W | N | S,
    9: N | E | S,
}


def _label_from_mask(mask: int) -> str:
    parts: list[str] = []
    if mask & N:
        parts.append("top")
    if mask & E:
        parts.append("right")
    if mask & S:
        parts.append("bottom")
    if mask & W:
        parts.append("left")
    if mask in (N | S, W | E):
        return f"straight ({'+'.join(parts)})"
    if len(parts) == 2:
        return f"corner ({'+'.join(parts)})"
    if len(parts) == 3:
        return f"T-junction ({'+'.join(parts)})"
    return "+".join(parts) or "empty"


TILE_STATES: dict[int, dict[str, int | str]] = {
    tile_id: {"mask": mask, "label": _label_from_mask(mask)}
    for tile_id, mask in MASK_BY_ID.items()
}


def mask_to_sides(mask: int) -> str:
    sides: list[str] = []
    if mask & N:
        sides.append("top")
    if mask & E:
        sides.append("right")
    if mask & S:
        sides.append("bottom")
    if mask & W:
        sides.append("left")
    return "+".join(sides) or "none"


def format_board_for_model(board: list[list[int]]) -> str:
    lines = [
        "Each cell is a connector tile. The number is an orientation id (0-9).",
        "One clockwise rotation: id becomes ROTATE_CW[id].",
        "Wires connect when touching sides both have an open edge.",
        "",
    ]
    for row in range(3):
        for col in range(3):
            tile_id = board[row][col]
            state = TILE_STATES[tile_id]
            cell = f"{row + 1}x{col + 1}"
            lines.append(
                f"{cell}: id={tile_id}, {state['label']}, sides={mask_to_sides(state['mask'])}"
            )
    return "\n".join(lines)


def rotation_steps(from_id: int, to_id: int) -> int:
    if from_id == to_id:
        return 0
    seen = {from_id}
    current = from_id
    for step in range(1, 5):
        current = ROTATE_CW[current]
        if current == to_id:
            return step
        if current in seen:
            return -1
        seen.add(current)
    return -1
