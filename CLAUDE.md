# CLAUDE.md

- Do not end a report with an offer, a suggestion or a question about further work the user did not ask for, such as staging a file, committing, or making another change. Report what was done and what was verified, and stop there.
- Do not mention a physical device in a report, such as saying that something was or was not checked on one. State only what was verified.
- Do not mention whether changes are committed or uncommitted in a report. Say only what was changed and what was verified.
- Do not append an `echo` that announces the end of a command, such as `; echo finished` or `; echo done`. A background task already reports its completion, and a foreground command returns when it ends.
- Do not open a pull request on a repository owned by an external team or account, such as the upstream of a fork, unless explicitly instructed to. Open pull requests only on repositories the user owns.
- Do not post a comment on a pull request, such as when closing one or after changing one, unless explicitly instructed to.
- Do not copy packages out of the pub local store (`~/.pub-cache`) into the repository, and do not modify the files inside it. When a dependency needs a change, make it in the dependency's own repository, added as a submodule.
