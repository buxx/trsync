Feature: SynchronizeFilledWorkspace
    Synchronize one filled workspace

    Scenario: Synchronize from existing filled workspace
        Given I have a fresh Tracim instance
        And I'm the user "user1"
        And I own the workspace "MyWorskpace1"
        And The workspace is filled with contents called "Set1"

        When I start and wait the end of synchronization

        Then I should see the trsync database file
        And Local folder contains "Set1"
        And Remote workspace contains "Set1"
