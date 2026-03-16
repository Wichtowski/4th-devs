import csv
import io
from typing import Type, TypeVar

T = TypeVar("T")

class CsvService:
    @staticmethod
    def read_records(csv_text: str) -> list[dict[str, str]]:
        reader = csv.DictReader(io.StringIO(csv_text))
        return [dict(row) for row in reader]

    @staticmethod
    def read_records_as(csv_text: str, cls: Type[T], **kwargs) -> list[T]:
        """Read CSV and convert each row to an instance of `cls` (e.g. a dataclass)."""
        records = CsvService.read_records(csv_text)
        return [cls(**row) for row in records]

