# 我们自己的 FigCraft 仓库

[English](../our-fork.md)

本地远程已经设好，不要再改：

| 远程 | 地址 | 用途 |
|---|---|---|
| `origin` | https://github.com/sn845fqg6k-cpu/figcraft.git | 我们的仓库。只往这里推送 |
| `upstream` | https://github.com/storytold/figcraft.git | 上游主仓库。只从这里拉取，不要推送 |

当前分支是 `main`，跟踪 `origin/main`。下面的命令都在仓库根目录、PowerShell 里执行。

## 软件自己检查更新

桌面版启动时的检查，和菜单里的检查更新，读的是 `apps/figcraft/config` 里当前环境的 `update_repo`。三份配置都是 `sn845fqg6k-cpu/figcraft`，所以它看我们仓库的 GitHub Release，不再看上游 `storytold/figcraft`。我们的仓库上还没有更高版本的 Release 时，检查会说已经是最新。改 `update_repo` 之后要重新编译。

## 把我们的改进推上去

先看改了什么：

```powershell
git status
```

提交并推到我们的仓库：

```powershell
git add .
git commit -m "这里写这次改了什么"
git push origin main
```

`git push` 不带参数也会推到 `origin`，也就是我们的仓库，不会推到上游。

## 上游更新后，合并进我们的仓库

```powershell
git fetch upstream
git merge upstream/main
git push origin main
```

合并前如果本地有没提交的修改，先按上一节提交，再合并。

## 合并出了冲突

Git 会标出冲突文件。打开这些文件，删掉 `<<<<<<<`、`=======`、`>>>>>>>` 这三行标记，留下要保留的内容，然后：

```powershell
git add .
git commit
git push origin main
```

这次 `git commit` 不用再写说明，Git 会用它准备好的合并说明。

想放弃这一次合并、回到合并前：

```powershell
git merge --abort
```

## 不要做的事

- 不要 `git push upstream`。我们没有上游的推送权限，那样也会把我们的改动送去主仓库。
- 不要 `git push --force`。会改写已经推上去的历史。
- 不要改 `git config` 里的用户名和邮箱，除非你自己要换提交身份。
