Feature: SynchronizeWorkspace
    Synchronize one workspace

    Scenario: Synchronize from existing empty workspace
        Given I have a fresh Tracim instance
        And I'm the user "user1"
        And I own the workspace "MyWorskpace1"

        When For workspace "MyWorskpace1", I start and wait the end of synchronization

        Then In workspace "MyWorskpace1", I should see the trsync database file
        And In workspace "MyWorskpace1", Local folder is empty
        And In workspace "MyWorskpace1", Remote workspace is empty
