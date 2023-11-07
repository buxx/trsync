import os
from pathlib import Path
import signal
from pytest_bdd import when, parsers

from tests.fixtures.base import (
    execute_trsync,
    execute_trsync_and_wait_finished,
)
from tests.fixtures.model import User, Workspace


@when("I start and wait the end of synchronization")
def sync_and_wait(
    user: User,
    workspace: Workspace,
    tmp_path: Path,
    container_port: int,
):
    with open(tmp_path / "trsync.log", "w+") as trsync_logs:
        execute_trsync_and_wait_finished(
            container_port=container_port,
            folder=workspace.folder(tmp_path),
            workspace_id=workspace.id,
            user=user,
            stdout=trsync_logs,
        )


@when("I start synchronization")
def start_sync(
    user: User,
    workspace: Workspace,
    tmp_path: Path,
    container_port: int,
):
    with open(tmp_path / "trsync.log", "w+") as trsync_logs:
        execute_trsync(
            container_port=container_port,
            folder=workspace.folder(tmp_path),
            workspace_id=workspace.id,
            user=user,
            stdout=trsync_logs,
        )


@when(
    parsers.cfparse('create local file at "{path}" with content "{content}"'),
)
def create_local_file(
    user: User, workspace: Workspace, path: str, content: str, tmp_path: Path
) -> Workspace:
    (workspace.folder(tmp_path) / str(path)[1:]).write_text(content)


@when(
    parsers.cfparse('create local folder at "{path}"'),
)
def create_local_folder(
    user: User, workspace: Workspace, path: str, tmp_path: Path
) -> Workspace:
    (workspace.folder(tmp_path) / str(path)[1:]).mkdir(parents=True)
