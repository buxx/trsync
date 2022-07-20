import dataclasses
from pathlib import Path


@dataclasses.dataclass
class User:
    username: str
    email: str
    password: str


@dataclasses.dataclass
class Workspace:
    id: int
    name: str

    def folder(self, tmp_path: Path) -> Path:
        path = tmp_path / self.name
        path.mkdir(parents=True, exist_ok=True)
        return path


@dataclasses.dataclass
class Content:
    id: int
    revision: int
    content: bytes
