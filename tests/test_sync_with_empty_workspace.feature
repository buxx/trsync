Feature: SynchronizeWithWorkspace
    Synchronize one workspace and make live manipulations

    Scenario: Synchronize from existing empty workspace and create files and folders
        Given I have a fresh Tracim instance
        And I'm the user "user1"
        And I own the workspace "MyWorskpace1"

        When For workspace "MyWorskpace1", I start synchronization
        And In workspace "MyWorskpace1", create local file at "/toto.txt" with content "toto"
        And In workspace "MyWorskpace1", create local folder at "/MyFolder"
        And In workspace "MyWorskpace1", create local file at "/MyFolder/toto2.txt" with content "toto2"

        Then In workspace "MyWorskpace1", I should see remote file at "/toto.txt"
        And In workspace "MyWorskpace1", I should see remote folder at "/MyFolder"
        And In workspace "MyWorskpace1", I should see remote file at "/MyFolder/toto2.txt"
