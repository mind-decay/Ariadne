import json
from pathlib import Path


class Counter:
    def __init__(self, start: int = 0) -> None:
        self.value = start

    def increment(self) -> int:
        self.value += 1
        return self.value


def make_counter(start: int) -> Counter:
    return Counter(start)


def load_config(path: str) -> dict:
    text = Path(path).read_text()
    return json.loads(text)
