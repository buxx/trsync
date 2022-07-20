from pytest_bdd import scenario


@scenario(
    "test_sync_with_conflicts.feature",
    "Synchronize with both modified file",
)
def test_sync_with_conflicts__both_modified():
    pass


@scenario(
    "test_sync_with_conflicts.feature",
    "Synchronize with locally deleted file",
)
def test_sync_with_conflicts__locally_deleted():
    pass
