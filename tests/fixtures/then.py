from pathlib import Path

from pytest_bdd import parsers, then
from tests.fixtures.base import (
    check_until,
    get_content_bytes,
    get_folder_listing,
    get_workspace_listing,
    get_content,
)

from tests.fixtures.model import User, Workspace
from tests.fixtures.sets import SETS


@then("I should see the trsync database file")
def database_file_exist(user: User, workspace: Workspace, tmp_path: Path):
    assert (tmp_path / workspace.folder(tmp_path) / ".trsync.db").exists()


@then("Local folder is empty")
def assert_local_folder_empty(user: User, workspace: Workspace, tmp_path: Path):
    assert get_folder_listing(workspace.folder(tmp_path)) == []


@then("Remote workspace is empty")
def remote_workspace_empty(user: User, workspace: Workspace):
    assert get_workspace_listing(user, workspace) == {}


@then(parsers.cfparse('Local folder contains "{set_name}"'))
def folder_contains_remove_contents1(
    user: User, workspace: Workspace, set_name: str, tmp_path: Path
):
    assert get_folder_listing(workspace.folder(tmp_path)) == list(
        sorted(SETS[set_name])
    )


@then(parsers.cfparse('Remote workspace contains "{set_name}"'))
def workspace_contains_remove_contents1(
    user: User, workspace: Workspace, set_name: str
):
    workspace_listing = get_workspace_listing(user, workspace).keys()
    assert list(sorted(workspace_listing)) == sorted(SETS[set_name])


@then(parsers.cfparse('I should see remote file at "{path}"'))
def workspace_contains_file(user: User, workspace: Workspace, path: str):
    def check():
        assert path in list(get_workspace_listing(user, workspace).keys())

    check_until(check)


@then(parsers.cfparse('I should not see remote file at "{path}"'))
def workspace_not_contains_file(user: User, workspace: Workspace, path: str):
    def check():
        assert path not in list(get_workspace_listing(user, workspace).keys())

    check_until(check)


@then(parsers.cfparse('I should see remote file "{path}" with content "{content}"'))
def workspace_contains_file_with_content(
    user: User, workspace: Workspace, path: str, content: str
):
    def check():
        workspace_listing = get_workspace_listing(user, workspace)
        assert path in list(workspace_listing.keys())
        content_id = workspace_listing[path]
        content_ = get_content_bytes(user, content_id)
        assert content_ == content.encode()

    check_until(check)


@then(parsers.cfparse('I should see remote folder at "{path}"'))
def workspace_contains_folder(user: User, workspace: Workspace, path: str):
    def check():
        assert path in list(get_workspace_listing(user, workspace).keys())

    check_until(check)


@then(parsers.cfparse('I should see local file "{path}" with content "{content}"'))
def local_file_with_content(
    tmp_path: Path, workspace: Workspace, path: str, content: str
):
    def check():
        path_ = tmp_path / workspace.folder(tmp_path) / path.strip("/")
        assert path_.exists()
        assert path_.read_bytes() == content.encode()

    check_until(check)


@then(parsers.cfparse('I should not see local file at "{path}"'))
def local_file_not_here(tmp_path: Path, workspace: Workspace, path: str):
    def check():
        path_ = tmp_path / workspace.folder(tmp_path) / path.strip("/")
        assert not path_.exists()

    check_until(check)
