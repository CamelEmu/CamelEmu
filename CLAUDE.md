# CLAUDE.md

## No unnecessary proposals

Do not end a report with an offer, a suggestion or a question about further work the user did not ask for, such as staging a file, committing, or making another change. Report what was done and what was verified, and stop there.

## No mention of a physical device

Do not mention a physical device in a report, such as saying that something was or was not checked on one. State only what was verified.

## No mention of commit state

Do not mention whether changes are committed or uncommitted in a report. Say only what was changed and what was verified.

## Do not copy or modify the pub local store

Do not copy packages out of the pub local store (`~/.pub-cache`) into the repository, and do not modify the files inside it. When a dependency needs a change, make it in the dependency's own repository, added as a submodule.
