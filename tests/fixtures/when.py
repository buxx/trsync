import os
from pathlib import Path
import typing
from pytest_bdd import when, parsers
import tests.fixtures.base as base

from tests.fixtures.base import (
    execute_trsync,
    execute_trsync_and_wait_finished,
)
from tests.fixtures.model import User, Workspace
from tests.fixtures.sets import (
    change_remote_file_workspace,
    create_remote,
    delete_remote,
    rename_remote_file,
)


@when(
    parsers.cfparse(
        'For workspace "{workspace_name}", '
        "I start and wait the end of synchronization"
    )
)
def sync_and_wait(
    user: User,
    workspace_name: str,
    tmp_path: Path,
    container_port: int,
):
    workspace = base.get_workspace_by_name(container_port, user, workspace_name)
    with open(tmp_path / "trsync.log", "w+") as trsync_logs:
        execute_trsync_and_wait_finished(
            container_port=container_port,
            folder=workspace.folder(tmp_path),
            workspace_id=workspace.id,
            user=user,
            stdout=trsync_logs,
        )


@when(parsers.cfparse('For workspace "{workspace_name}", I start synchronization'))
def start_sync(
    user: User,
    workspace_name: str,
    tmp_path: Path,
    container_port: int,
    request,
):
    log_path = os.environ.get("TRSYNC_LOG_PATH", tmp_path / "trsync.log")
    workspace = base.get_workspace_by_name(container_port, user, workspace_name)
    with open(log_path, "w+") as trsync_logs:
        trsync_pids = execute_trsync(
            container_port=container_port,
            folder=workspace.folder(tmp_path),
            workspace_id=workspace.id,
            user=user,
            stdout=trsync_logs,
        )
    request.node.user_properties.append(("trsync_pid", trsync_pids))


@when(
    parsers.cfparse(
        'In workspace "{workspace_name}", create local file at "{path}" with content "{content}"'
    ),
)
def create_local_file(
    user: User,
    workspace_name: str,
    path: str,
    content: str,
    tmp_path: Path,
    container_port: int,
) -> Workspace:
    workspace = base.get_workspace_by_name(container_port, user, workspace_name)
    (workspace.folder(tmp_path) / str(path)[1:]).write_text(content)


@when(
    parsers.cfparse(
        'In workspace "{workspace_name}", create remote file "{path}" with content "{content}"'
    ),
)
def create_remote_folder(
    user: User,
    workspace_name: str,
    path: str,
    content: str,
    tmp_path: Path,
    container_port: int,
    content_ids: typing.Dict[str, int],
) -> Workspace:
    workspace = base.get_workspace_by_name(container_port, user, workspace_name)
    create_remote(
        container_port=container_port,
        user=user,
        workspace=workspace,
        file_path=path,
        content_ids=content_ids,
        content=content,
    )


@when(
    parsers.cfparse('In workspace "{workspace_name}", delete remote file "{path}"'),
)
def delete_remote_file(
    user: User,
    workspace_name: str,
    path: str,
    container_port: int,
    content_ids: typing.Dict[str, int],
) -> Workspace:
    workspace = base.get_workspace_by_name(container_port, user, workspace_name)
    delete_remote(
        container_port=container_port,
        user=user,
        workspace=workspace,
        file_path=path,
        content_ids=content_ids,
    )


@when(
    parsers.cfparse('In workspace "{workspace_name}", create remote folder "{path}"'),
)
def create_remote_folder(
    user: User,
    workspace_name: str,
    path: str,
    content: str,
    tmp_path: Path,
    container_port: int,
    content_ids: typing.Dict[str, int],
) -> Workspace:
    workspace = base.get_workspace_by_name(container_port, user, workspace_name)
    create_remote(
        container_port=container_port,
        user=user,
        workspace=workspace,
        file_path=path,
        content_ids=content_ids,
        content=content,
    )


@when(
    parsers.cfparse('In workspace "{workspace_name}", create local folder at "{path}"'),
)
def create_local_folder(
    user: User,
    workspace_name: str,
    path: str,
    tmp_path: Path,
    container_port: int,
) -> Workspace:
    workspace = base.get_workspace_by_name(container_port, user, workspace_name)
    (workspace.folder(tmp_path) / str(path)[1:]).mkdir(parents=True)


@when(
    parsers.cfparse(
        'In workspace "{workspace_name1}", '
        'I rename remote file "{file_name1}" '
        'into "{file_name2}" '
        'in "{workspace_name2}"'
    )
)
def move_remote_file_in_workspace(
    container_port: int,
    user: User,
    content_ids: dict,
    workspace_name1: str,
    workspace_name2: str,
    file_name1: str,
    file_name2: str,
    content: str,
) -> None:
    content_id = content_ids[file_name1]
    workspace1 = base.get_workspace_by_name(container_port, user, workspace_name1)
    workspace2 = base.get_workspace_by_name(container_port, user, workspace_name2)

    change_remote_file_workspace(
        container_port,
        user,
        content_id,
        workspace1.id,
        workspace2.id,
    )

    if file_name1 != file_name2:
        rename_remote_file(
            container_port,
            user,
            content_id,
            workspace2.id,
            file_name2,
        )
