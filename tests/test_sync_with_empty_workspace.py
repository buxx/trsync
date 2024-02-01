from pytest_bdd import scenario


@scenario(
    "test_sync_with_empty_workspace.feature",
    "Synchronize from existing empty workspace and create files and folders on local",
)
def test_sync_with_empty_workspace_and_create_on_remote():
    pass


@scenario(
    "test_sync_with_empty_workspace.feature",
    "Synchronize from existing empty workspace and create files and folders on remote",
)
def test_sync_with_empty_workspace_and_create_on_local():
    pass
