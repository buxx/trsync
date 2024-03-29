Feature: MoveFromWorkspaceToAnother
    Move files from workspace to another in different conditions

    Scenario: Moved file from workspace to another what I own, when offline
        Given I have a fresh Tracim instance
        And I'm the user "user1"
        And I own the workspace "MyWorskpace1"
        And I own the workspace "MyWorskpace2"
        And In workspace "MyWorskpace1", I create remote file "/file_toto.txt" with content "Hello World"
        And For workspace "MyWorskpace1", I start and wait the end of synchronization
        And In workspace "MyWorskpace1", I rename remote file "/file_toto.txt" into "/file_toto.txt" in "MyWorskpace2"

        When For workspace "MyWorskpace1", I start and wait the end of synchronization

        Then In workspace "MyWorskpace1", I should not see remote file "/file_toto.txt"
        And In workspace "MyWorskpace2", I should see remote file "/file_toto.txt"

    Scenario: Moved file from workspace to another what I own, when online
        Given I have a fresh Tracim instance
        And I'm the user "user1"
        And I own the workspace "MyWorskpace1"
        And I own the workspace "MyWorskpace2"
        And In workspace "MyWorskpace1", I create remote file "/file_toto.txt" with content "Hello World"

        When For workspace "MyWorskpace1", I start synchronization
        And In workspace "MyWorskpace1", I rename remote file "/file_toto.txt" into "/file_toto.txt" in "MyWorskpace2"

        Then In workspace "MyWorskpace1", I should not see remote file "/file_toto.txt"
        And In workspace "MyWorskpace2", I should see remote file "/file_toto.txt"

    Scenario: Moved file from workspace to another what I own, when offline, by syncing both
        Given I have a fresh Tracim instance
        And I'm the user "user1"
        And I own the workspace "MyWorskpace1"
        And I own the workspace "MyWorskpace2"
        And In workspace "MyWorskpace1", I create remote file "/file_toto.txt" with content "Hello World"
        And For workspace "MyWorskpace1", I start and wait the end of synchronization
        And In workspace "MyWorskpace1", I rename remote file "/file_toto.txt" into "/file_toto.txt" in "MyWorskpace2"

        When For workspace "MyWorskpace1", I start and wait the end of synchronization
        And For workspace "MyWorskpace2", I start and wait the end of synchronization

        Then In workspace "MyWorskpace1", I should not see remote file "/file_toto.txt"
        And In workspace "MyWorskpace1", I should not see local file "/file_toto.txt"
        And In workspace "MyWorskpace2", I should see remote file "/file_toto.txt"
        And In workspace "MyWorskpace2", I should see local file "/file_toto.txt"

    Scenario: Moved file from workspace to another what I own, when online, by syncing both
        Given I have a fresh Tracim instance
        And I'm the user "user1"
        And I own the workspace "MyWorskpace1"
        And I own the workspace "MyWorskpace2"
        And In workspace "MyWorskpace1", I create remote file "/file_toto.txt" with content "Hello World"

        When For workspace "MyWorskpace1", I start synchronization
        And For workspace "MyWorskpace2", I start synchronization
        And In workspace "MyWorskpace1", I rename remote file "/file_toto.txt" into "/file_toto.txt" in "MyWorskpace2"

        Then In workspace "MyWorskpace1", I should not see remote file "/file_toto.txt"
        And In workspace "MyWorskpace1", I should not see local file "/file_toto.txt"
        And In workspace "MyWorskpace2", I should see remote file "/file_toto.txt"
        And In workspace "MyWorskpace2", I should see local file "/file_toto.txt"
