# Our FigCraft fork

[中文](zh-CN/our-fork.md)

The local remotes are already set. Leave them as they are:

| Remote | URL | Use |
|---|---|---|
| `origin` | https://github.com/sn845fqg6k-cpu/figcraft.git | Our repository. Push only here |
| `upstream` | https://github.com/storytold/figcraft.git | Upstream. Pull from here. Do not push |

The current branch is `main`, tracking `origin/main`. Run the commands below from the repository root in PowerShell.

## Update checks

The desktop check at startup, and Check for updates in the menu, read `update_repo` from the environment file under `apps/figcraft/config`. All three name `sn845fqg6k-cpu/figcraft`, so the app asks our GitHub Releases and not upstream `storytold/figcraft`. With no newer Release on our repository, the check reports that this copy is current. A change to `update_repo` needs a rebuild.

## Push our changes

See what changed:

```powershell
git status
```

Commit and push to our repository:

```powershell
git add .
git commit -m "what changed this time"
git push origin main
```

`git push` with no arguments also goes to `origin`, our repository. It does not go to upstream.

## Merge upstream updates into our repository

```powershell
git fetch upstream
git merge upstream/main
git push origin main
```

If you have uncommitted changes, commit them with the steps above before merging.

## Conflicts

Git names the conflicted files. Open them, delete the `<<<<<<<`, `=======` and `>>>>>>>` markers, and keep the text you want. Then:

```powershell
git add .
git commit
git push origin main
```

This `git commit` needs no message. Git already prepared the merge message.

To drop the merge and return to the state before it:

```powershell
git merge --abort
```

## Do not

- Do not `git push upstream`. We have no push access there, and it would send our changes to the main repository.
- Do not `git push --force`. It rewrites history that is already pushed.
- Do not change the user name or email in `git config` unless you mean to change the commit identity.
