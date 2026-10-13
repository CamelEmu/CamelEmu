# CLAUDE.md

## No unnecessary proposals

Do not end a report with an offer, a suggestion or a question about further work the user did not ask for, such as staging a file, committing, or making another change. Report what was done and what was verified, and stop there.

## No mention of a physical device

Do not mention a physical device in a report, such as saying that something was or was not checked on one. State only what was verified.

## No mention of commit state

Do not mention whether changes are committed or uncommitted in a report. Say only what was changed and what was verified.

## Do not open pull requests on external repositories

Do not open a pull request on a repository owned by an external team or account, such as the upstream of a fork, unless explicitly instructed to. Open pull requests only on repositories the user owns.

## Do not comment on pull requests

Do not post a comment on a pull request, such as when closing one or after changing one, unless explicitly instructed to.

## Do not copy or modify the pub local store

Do not copy packages out of the pub local store (`~/.pub-cache`) into the repository, and do not modify the files inside it. When a dependency needs a change, make it in the dependency's own repository, added as a submodule.

## Do not filter output by guessed patterns

Do not filter the output of a build, a log or another command with patterns guessed in advance, such as `grep -E 'Built build|error:|FAILURE'`. Read the output itself with the Read tool, and judge the result from what it actually says.

## Do not edit files with commands or scripts

Do not edit source files with `sed`, `awk`, `perl`, Python, JavaScript (Node.js) or another command or script. Read each file with the Read tool and change it with the Edit or Write tool, so that every change is made on purpose and can be reviewed.
