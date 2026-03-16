import sys
import importlib
from pathlib import Path

from dotenv import load_dotenv

_ROOT_DIR = Path(__file__).resolve().parent
_SRC_DIR = _ROOT_DIR / "src"
if _SRC_DIR.exists():
    sys.path.insert(0, str(_SRC_DIR))

_EXERCISES_DIR = Path(__file__).resolve().parent.parent
load_dotenv(_EXERCISES_DIR / ".env")

EXERCISES = {
    "1": "s02_python.E01",
    "2": "s02_python.E02",
    "3": "s02_python.E03",
    "4": "s02_python.E04",
    "5": "s02_python.E05",
}

def main():
    if len(sys.argv) != 2 or sys.argv[1] not in EXERCISES:
        print(f"Usage: uv run main.py <exercise_number>")
        print(f"Available exercises: {', '.join(sorted(EXERCISES.keys()))}")
        sys.exit(1)

    exercise_id = sys.argv[1]
    module_path = EXERCISES[exercise_id]

    print(f"▶ Running exercise {exercise_id} ({module_path})")
    module = importlib.import_module(module_path)
    module.run()

if __name__ == "__main__":
    main()