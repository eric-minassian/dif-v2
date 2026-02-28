The AI agent has full access and control over the `.agents` folder.

It may create, organize, update, and delete anything there as needed to help across sessions.

## Git Workflow

We use git worktrees, so each session already has its own branch. When committing and pushing changes:

1. Commit the changes on the current worktree branch.
2. Push the branch.
3. Open a pull request using `gh pr create`.
4. Rebase and merge the PR into main using `gh pr merge --rebase --delete-branch`.

When the user says **"ship it"**, run the full workflow: commit, push, open PR, and rebase-merge into main.
