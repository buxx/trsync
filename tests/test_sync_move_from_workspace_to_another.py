from pytest_bdd import scenario


@scenario(
    "test_sync_move_from_workspace_to_another.feature",
    "Moved file from workspace to another what I own, when offline",
)
def test_sync_move_from_workspace_to_another__when_offline():
    pass


@scenario(
    "test_sync_move_from_workspace_to_another.feature",
    "Moved file from workspace to another what I own, when online",
)
def test_sync_move_from_workspace_to_another__when_online():
    pass


@scenario(
    "test_sync_move_from_workspace_to_another.feature",
    "Moved file from workspace to another what I own, when offline, by syncing both",
)
def test_sync_move_from_workspace_to_another__when_online__by_syncing_both():
    pass
