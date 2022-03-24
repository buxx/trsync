Feature: SynchronizeWorkspace
    Synchronize one workspace

    Scenario: Synchronize from existing empty workspace
        Given I have a fresh Tracim instance
        And I'm the user "user1"
        And I own the workspace "MyWorskpace1"

        When I start and wait the end of synchronization

        Then I should see the trsync database file
        And Local folder is empty
        And Remote workspace is empty
