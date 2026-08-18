# Undoing things in Git

## Not committed yet

`git restore <file>` throws the change away and there is no going back.
`git restore --staged <file>` only takes it out of staging and keeps the text —
that is the one that serves most of the time.

## Committed, not pushed

`git commit --amend` fixes the commit on top: wrong message, forgotten file.
`git reset --soft HEAD~1` takes the commit apart and puts everything back in
staging, which helps when it bundled two things that should have been separate.

## Already pushed

Now it is not erased, it is corrected on top. `git revert <commit>` writes a new
commit that undoes the old one, and the history keeps telling the truth.
Rewriting what is already on the server breaks the clone of whoever pulled.

## What gets forgotten

`git reflog` keeps every place HEAD pointed over the last few days. A "lost"
commit is almost never lost — it is there, unnamed, waiting.
