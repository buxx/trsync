from pytest_bdd import parsers, given

from tests.fixtures.model import User, Workspace
import tests.fixtures.base as base
from tests.fixtures.sets import create_set_on_remote


@given("I have a fresh Tracim instance")
def fresh_instance() -> None:
    base.stopped_tracim_instance()
    base.fresh_tracim_instance()
    base.ensure_users()


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
def workspace(user: User, name: str) -> Workspace:
    return base.create_workspace(user, name)


@given(parsers.cfparse('The workspace is filled with contents called "{set_name}"'))
def workspace_filled_with_set(user: User, workspace: Workspace, set_name: str) -> None:
    create_set_on_remote(user, workspace, set_name)
