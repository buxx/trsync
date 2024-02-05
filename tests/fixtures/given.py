from pathlib import Path
import pathlib
from pytest import FixtureRequest
from pytest_bdd import parsers, given

from tests.fixtures.model import User, Workspace
import tests.fixtures.base as base
from tests.fixtures.sets import (
    change_remote_file_workspace,
    create_set_on_remote,
    create_remote,
    rename_remote_file,
    update_remote_file,
)


@given("I have a fresh Tracim instance")
def fresh_instance(request: FixtureRequest, container_port: int) -> None:
    container_name = f"{base.TRACIM_CONTAINER_NAME}-{request.node.name}"
    base.stopped_tracim_instance(container_name)
    base.fresh_tracim_instance(container_name, container_port)
    base.ensure_users(container_port)


@given(
    parsers.cfparse('I\'m the user "{username}"'),
    target_fixture="user",
)
def current_user(username: str) -> User:
    return base.USERS[username]


@given(
    parsers.cfparse('I own the workspace "{name}"'),
    target_fixture="workspace",
)
def owned_workspace(user: User, name: str, container_port: int) -> None:
    base.create_workspace(container_port, user, name)


@given(
    parsers.cfparse(
        'For workspace "{workspace_name}", '
        'The workspace is filled with contents called "{set_name}"'
    )
)
def workspace_filled_with_set(
    container_port: int,
    user: User,
    workspace_name: str,
    set_name: str,
) -> None:
    workspace = base.get_workspace_by_name(container_port, user, workspace_name)
    create_set_on_remote(container_port, user, workspace, set_name)


@given(
    parsers.cfparse(
        'In workspace "{workspace_name}", '
        'I create remote file "{file_name}" '
        'with content "{content}"'
    ),
    target_fixture="content_ids",
)
def create_remote_file(
    user: User,
    workspace_name: str,
    file_name: str,
    content: str,
    container_port: int,
) -> None:
    content_ids = {}
    workspace = base.get_workspace_by_name(container_port, user, workspace_name)
    create_remote(
        container_port,
        user,
        workspace,
        file_name,
        content_ids,
        content=content,
    )
    return content_ids


@given(
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
    with open(tmp_path / f"{workspace_name}_trsync.log", "a+") as trsync_logs:
        base.execute_trsync_and_wait_finished(
            container_port=container_port,
            folder=workspace.folder(tmp_path),
            workspace_id=workspace.id,
            user=user,
            stdout=trsync_logs,
        )


@given(
    parsers.cfparse(
        'In workspace "{workspace_name}", '
        'I update remote file "{file_name}" '
        'with content "{content}"'
    )
)
def update_remote_file_with_content(
    container_port: int,
    user: User,
    content_ids: dict,
    workspace_name: str,
    file_name: str,
    content: str,
) -> None:
    content_id = content_ids[file_name]
    workspace = base.get_workspace_by_name(container_port, user, workspace_name)
    update_remote_file(
        container_port,
        user,
        workspace,
        content_id=content_id,
        name=pathlib.Path(file_name).name,
        content=content,
    )


@given(
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


@given(
    parsers.cfparse(
        'In workspace "{workspace_name}", '
        'I update local file "{file_name}" '
        'with content "{content}"'
    )
)
def update_local_file(
    container_port: int,
    user: User,
    tmp_path: Path,
    workspace_name: str,
    file_name: str,
    content: str,
) -> None:
    workspace = base.get_workspace_by_name(container_port, user, workspace_name)
    (
        tmp_path / workspace.folder(tmp_path) / pathlib.Path(file_name.strip("/"))
    ).write_bytes(content.encode())


@given(
    parsers.cfparse(
        'In workspace "{workspace_name}", I delete local file "{file_name}"'
    )
)
def delete_local_file(tmp_path: Path, workspace_name: str, file_name: str) -> None:
    (tmp_path / workspace_name / pathlib.Path(file_name.strip("/"))).unlink()
