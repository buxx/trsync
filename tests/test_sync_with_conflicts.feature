Feature: SynchronizeWithConflicts
    Conflicts situations

    Scenario: Synchronize with both modified file
        Given I have a fresh Tracim instance
        And I'm the user "user1"
        And I own the workspace "MyWorskpace1"
        And In workspace "MyWorskpace1", I create remote file "/file_toto.txt" with content "Hello World"
        And For workspace "MyWorskpace1", I start and wait the end of synchronization
        And In workspace "MyWorskpace1", I update remote file "/file_toto.txt" with content "Hello World 2"
        And In workspace "MyWorskpace1", I update local file "/file_toto.txt" with content "Hello World 3"

        When For workspace "MyWorskpace1", I start and wait the end of synchronization

        Then In workspace "MyWorskpace1", I should see remote file "/file_toto.txt" with content "Hello World 3"
        And In workspace "MyWorskpace1", I should see local file "/file_toto.txt" with content "Hello World 3"

    Scenario: Synchronize with locally deleted file
        Given I have a fresh Tracim instance
        And I'm the user "user1"
        And I own the workspace "MyWorskpace1"
        And In workspace "MyWorskpace1", I create remote file "/file_toto.txt" with content "Hello World"
        And For workspace "MyWorskpace1", I start and wait the end of synchronization
        And In workspace "MyWorskpace1", I update remote file "/file_toto.txt" with content "Hello World 2"
        And In workspace "MyWorskpace1", I delete local file "/file_toto.txt"

        When For workspace "MyWorskpace1", I start and wait the end of synchronization

        Then In workspace "MyWorskpace1", I should not see remote file at "/file_toto.txt"
        And In workspace "MyWorskpace1", I should not see local file at "/file_toto.txt"
