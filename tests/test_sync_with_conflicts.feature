Feature: SynchronizeWithConflicts
    Conflicts situations

    Scenario: Synchronize with both modified file
        Given I have a fresh Tracim instance
        And I'm the user "user1"
        And I own the workspace "MyWorskpace1"
        And I create remote file "/file_toto.txt" with content "Hello World"
        And I start and wait the end of synchronization
        And I update remote file "/file_toto.txt" with content "Hello World 2"
        And I update local file "/file_toto.txt" with content "Hello World 3"

        When I start and wait the end of synchronization

        Then I should see remote file "/file_toto.txt" with content "Hello World 3"
        And I should see local file "/file_toto.txt" with content "Hello World 3"

    Scenario: Synchronize with locally deleted file
        Given I have a fresh Tracim instance
        And I'm the user "user1"
        And I own the workspace "MyWorskpace1"
        And I create remote file "/file_toto.txt" with content "Hello World"
        And I start and wait the end of synchronization
        And I update remote file "/file_toto.txt" with content "Hello World 2"
        And I delete local file "/file_toto.txt"

        When I start and wait the end of synchronization

        Then I should not see remote file at "/file_toto.txt"
        And I should not see local file at "/file_toto.txt"
