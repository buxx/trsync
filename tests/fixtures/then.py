from pathlib import Path

from pytest_bdd import parsers, then
from tests.fixtures.base import (
    check_until,
    get_content_bytes,
    get_folder_listing,
    get_workspace_listing,
)

import tests.fixtures.base as base
from tests.fixtures.model import User, Workspace
from tests.fixtures.sets import SETS


@then(
    parsers.cfparse(
        'In workspace "{workspace_name}", I should see the trsync database file'
    )
)
def database_file_exist(
    user: User,
    workspace_name: str,
    tmp_path: Path,
    container_port: int,
):
    workspace = base.get_workspace_by_name(container_port, user, workspace_name)
    assert (tmp_path / workspace.folder(tmp_path) / ".trsync.db").exists()


@then(parsers.cfparse('In workspace "{workspace_name}", Local folder is empty'))
def assert_local_folder_empty(
    user: User,
    workspace_name: str,
    tmp_path: Path,
    container_port: int,
):
    workspace = base.get_workspace_by_name(container_port, user, workspace_name)
    assert get_folder_listing(workspace.folder(tmp_path)) == []


@then(parsers.cfparse('In workspace "{workspace_name}", Remote workspace is empty'))
def remote_workspace_empty(
    user: User,
    workspace_name: str,
    container_port: int,
):
    workspace = base.get_workspace_by_name(container_port, user, workspace_name)
    assert get_workspace_listing(container_port, user, workspace) == {}


@then(
    parsers.cfparse(
        'In workspace "{workspace_name}", Local folder contains "{set_name}"'
    )
)
def folder_contains_remove_contents1(
    user: User,
    workspace_name: str,
    set_name: str,
    tmp_path: Path,
    container_port: int,
):
    workspace = base.get_workspace_by_name(container_port, user, workspace_name)
    assert get_folder_listing(workspace.folder(tmp_path)) == list(
        sorted(SETS[set_name])
    )


@then(
    parsers.cfparse(
        'In workspace "{workspace_name}", Remote workspace contains "{set_name}"'
    )
)
def workspace_contains_remove_contents1(
    user: User,
    workspace_name: str,
    set_name: str,
    container_port: int,
):
    workspace = base.get_workspace_by_name(container_port, user, workspace_name)
    workspace_listing = get_workspace_listing(container_port, user, workspace).keys()
    assert list(sorted(workspace_listing)) == sorted(SETS[set_name])


@then(
    parsers.cfparse(
        'In workspace "{workspace_name}", I should see remote file "{path}"'
    )
)
def workspace_contains_file(
    user: User,
    workspace_name: str,
    path: str,
    container_port: int,
):
    workspace = base.get_workspace_by_name(container_port, user, workspace_name)

    def check():
        keys = get_workspace_listing(container_port, user, workspace).keys()
        assert path in list(keys), f"{path} should be in {keys}"

    check_until(check)


@then(
    parsers.cfparse(
        'In workspace "{workspace_name}", I should not see remote file "{path}"'
    )
)
def workspace_not_contains_file(
    user: User,
    workspace_name: str,
    path: str,
    container_port: int,
):
    workspace = base.get_workspace_by_name(container_port, user, workspace_name)

    def check():
        assert path not in list(
            get_workspace_listing(container_port, user, workspace).keys()
        )

    check_until(check)


@then(
    parsers.cfparse(
        'In workspace "{workspace_name}", '
        'I should see remote file "{path}" '
        'with content "{content}"'
    )
)
def workspace_contains_file_with_content(
    user: User,
    workspace_name: str,
    path: str,
    content: str,
    container_port: int,
):
    workspace = base.get_workspace_by_name(container_port, user, workspace_name)

    def check():
        workspace_listing = get_workspace_listing(container_port, user, workspace)
        assert path in list(workspace_listing.keys())
        content_id = workspace_listing[path]
        content_ = get_content_bytes(container_port, user, content_id)
        assert content_ == content.encode()

    check_until(check)


@then(
    parsers.cfparse(
        'In workspace "{workspace_name}", I should see remote folder at "{path}"'
    )
)
def workspace_contains_folder(
    user: User,
    workspace_name: str,
    path: str,
    container_port: int,
):
    workspace = base.get_workspace_by_name(container_port, user, workspace_name)

    def check():
        listing = list(get_workspace_listing(container_port, user, workspace).keys())
        assert path in listing, f"'{path}' not found in remote ('{listing}')"

    check_until(check)


@then(
    parsers.cfparse(
        'In workspace "{workspace_name}", '
        'I should see local file "{path}" '
        'with content "{content}"'
    )
)
def local_file_with_content(
    user: User,
    container_port: int,
    tmp_path: Path,
    workspace_name: str,
    path: str,
    content: str,
):
    workspace = base.get_workspace_by_name(container_port, user, workspace_name)

    def check():
        path_ = tmp_path / workspace.folder(tmp_path) / path.strip("/")
        assert path_.exists(), f"{path_} file should exist"
        assert (
            path_.read_bytes() == content.encode()
        ), f"{path_} should contain '{content}', not '{path_.read_bytes().decode()}'"

    check_until(check)


@then(
    parsers.cfparse('In workspace "{workspace_name}", I should see local file "{path}"')
)
def local_file(
    user: User,
    container_port: int,
    tmp_path: Path,
    workspace_name: str,
    path: str,
):
    workspace = base.get_workspace_by_name(container_port, user, workspace_name)

    def check():
        path_ = tmp_path / workspace.folder(tmp_path) / path.strip("/")
        assert path_.exists(), f"{path_} file should exist"

    check_until(check)


@then(
    parsers.cfparse(
        'In workspace "{workspace_name}", I should see local folder "{path}"'
    )
)
def local_file(
    user: User,
    container_port: int,
    tmp_path: Path,
    workspace_name: str,
    path: str,
):
    workspace = base.get_workspace_by_name(container_port, user, workspace_name)

    def check():
        path_ = tmp_path / workspace.folder(tmp_path) / path.strip("/")
        assert path_.exists(), f"{path_} folder should exist"

    check_until(check)


@then(
    parsers.cfparse(
        'In workspace "{workspace_name}", I should not see local file "{path}"'
    )
)
def local_file_not_here(
    user: User,
    container_port: int,
    tmp_path: Path,
    workspace_name: str,
    path: str,
):
    workspace = base.get_workspace_by_name(container_port, user, workspace_name)

    def check():
        path_ = tmp_path / workspace.folder(tmp_path) / path.strip("/")
        assert not path_.exists()

    check_until(check)


@then(
    parsers.cfparse(
        'In workspace "{workspace_name}", I should not see local file "{path}" during {during} seconds'
    )
)
def local_file_not_here(
    user: User,
    container_port: int,
    tmp_path: Path,
    workspace_name: str,
    path: str,
    during: int,
):
    workspace = base.get_workspace_by_name(container_port, user, workspace_name)

    def check():
        path_ = tmp_path / workspace.folder(tmp_path) / path.strip("/")
        assert not path_.exists(), f"Path {path_} should not exist"

    check_until(check, during=during)
