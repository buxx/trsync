from pathlib import Path

from pytest_bdd import parsers, then
from tests.fixtures.base import check_until, get_folder_listing, get_workspace_listing

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
    assert get_workspace_listing(user, workspace) == []


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
    assert get_workspace_listing(user, workspace) == sorted(SETS[set_name])


@then(parsers.cfparse('I should see remote file at "{path}"'))
def workspace_contains_file(user: User, workspace: Workspace, path: str):
    def check():
        assert path in get_workspace_listing(user, workspace)

    check_until(check)


@then(parsers.cfparse('I should see remote folder at "{path}"'))
def workspace_contains_folder(user: User, workspace: Workspace, path: str):
    def check():
        assert path in get_workspace_listing(user, workspace)

    check_until(check)
