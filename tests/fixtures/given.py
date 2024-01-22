from pathlib import Path
import pathlib
from pytest import FixtureRequest
from pytest_bdd import parsers, given

from tests.fixtures.model import User, Workspace
import tests.fixtures.base as base
from tests.fixtures.sets import create_set_on_remote, create_remote, update_file


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
def workspace(user: User, name: str, container_port: int) -> Workspace:
    return base.create_workspace(container_port, user, name)


@given(parsers.cfparse('The workspace is filled with contents called "{set_name}"'))
def workspace_filled_with_set(
    container_port: int,
    user: User,
    workspace: Workspace,
    set_name: str,
) -> None:
    create_set_on_remote(container_port, user, workspace, set_name)


@given(
    parsers.cfparse('I create remote file "{file_name}" with content "{content}"'),
    target_fixture="content_ids",
)
def create_remote_file(
    user: User,
    workspace: Workspace,
    file_name: str,
    content: str,
    container_port: int,
) -> None:
    content_ids = {}
    create_remote(
        container_port,
        user,
        workspace,
        file_name,
        content_ids,
        contents={file_name: content},
    )
    return content_ids


@given("I start and wait the end of synchronization")
def sync_and_wait(
    user: User,
    workspace: Workspace,
    tmp_path: Path,
    container_port: int,
):
    with open(tmp_path / "trsync.log", "a+") as trsync_logs:
        base.execute_trsync_and_wait_finished(
            container_port=container_port,
            folder=workspace.folder(tmp_path),
            workspace_id=workspace.id,
            user=user,
            stdout=trsync_logs,
        )


@given(parsers.cfparse('I update remote file "{file_name}" with content "{content}"'))
def update_remote_file(
    container_port: int,
    user: User,
    workspace: Workspace,
    content_ids: dict,
    file_name: str,
    content: str,
) -> None:
    content_id = content_ids[file_name]
    update_file(
        container_port,
        user,
        workspace,
        content_id=content_id,
        name=pathlib.Path(file_name).name,
        content=content,
    )


@given(parsers.cfparse('I update local file "{file_name}" with content "{content}"'))
def update_local_file(
    tmp_path: Path, workspace: Workspace, file_name: str, content: str
) -> None:
    (
        tmp_path / workspace.folder(tmp_path) / pathlib.Path(file_name.strip("/"))
    ).write_bytes(content.encode())


@given(parsers.cfparse('I delete local file "{file_name}"'))
def delete_local_file(tmp_path: Path, workspace: Workspace, file_name: str) -> None:
    (
        tmp_path / workspace.folder(tmp_path) / pathlib.Path(file_name.strip("/"))
    ).unlink()
