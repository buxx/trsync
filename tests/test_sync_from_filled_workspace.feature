Feature: SynchronizeFilledWorkspace
    Synchronize one filled workspace

    Scenario: Synchronize from existing filled workspace
        Given I have a fresh Tracim instance
        And I'm the user "user1"
        And I own the workspace "MyWorskpace1"
        And For workspace "MyWorskpace1", The workspace is filled with contents called "Set1"

        When For workspace "MyWorskpace1", I start and wait the end of synchronization

        Then In workspace "MyWorskpace1", I should see the trsync database file
        And In workspace "MyWorskpace1", Local folder contains "Set1"
        And In workspace "MyWorskpace1", Remote workspace contains "Set1"
