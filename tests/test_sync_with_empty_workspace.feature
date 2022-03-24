Feature: SynchronizeWithWorkspace
    Synchronize one workspace and make live manipulations

    Scenario: Synchronize from existing empty workspace and create files and folders
        Given I have a fresh Tracim instance
        And I'm the user "user1"
        And I own the workspace "MyWorskpace1"

        When I start synchronization
        And create local file at "/toto.txt" with content "toto"
        And create local folder at "/MyFolder"
        And create local file at "/MyFolder/toto2.txt" with content "toto2"

        Then I should see remote file at "/toto.txt"
        And I should see remote folder at "/MyFolder"
        And I should see remote file at "/MyFolder/toto2.txt"
