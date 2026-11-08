# 个人版本与上游同步

## 分支职责

| 分支 / remote | 用途 |
| --- | --- |
| `upstream` | 原项目 `https://github.com/QingJ01/Pebble.git`，只拉取 |
| `origin` | 个人 fork `https://github.com/xkcore123/Pebble.git` |
| `master` | 上游镜像，只快进同步 `upstream/master`，不合入个人修复 |
| `personal` | 个人使用和云编译分支：上游代码 + 独立修复提交 |
| `fix/**` | 临时修复分支，今后从 `personal` 创建并合回 `personal` |

GitHub 默认分支保留 `master`，以便直接比较、同步上游。查看自己的修改请切换到
`personal`。PR 是分支间的合并请求，不是把仓库放进别人仓库；原 PR #1 已由
`personal` 接管，不应再合入 `master`。旧修复分支保留用于追溯。

## 同步操作

在仓库根目录运行（脚本不会上传代码，也不会使用强制覆盖）：

```powershell
git switch personal
node scripts/sync-upstream.mjs --check
node scripts/sync-upstream.mjs --apply
```

脚本拉取两个 remote，只接受干净工作区和可快进的镜像分支；先为个人版本创建
本地备份分支，再合入上游。成功后才更新本地 `master`。没有新提交时不会创建
无意义的 merge。不会推送上游标签，不会误触发 Release。

冲突时保留现场及备份，**不会自动选上游/个人任意一边**，不会更新 `master`
或远端。解决冲突并提交，或使用 `git merge --abort` 撤销本次合并，然后再同步。

检查差异、运行测试，再上传：

```powershell
git diff master...personal --stat
node --test scripts/sync-upstream-checks.mjs
pnpm test
pnpm build:frontend
# 配好 Rust 构建环境时运行：cargo test --workspace --all-targets --locked
git push --atomic origin master personal
```

普通 push 不会覆盖他人提交；远端有新变化时会拒绝推送。上传 `personal` 会
直接触发 **Personal Build** 的 Windows/macOS/Linux 格式检查、Clippy、测试和
打包，**不需要创建指向 master 的 PR**。在 Actions 对应运行的 Artifacts 下载。
没有创建定时同步任务；每次同步后仍需要解决可能的语义冲突并确认 CI 通过。

## 修复清单与上游吸收

- IMAP 断连重连、清理超时、同步状态：`e190b88`、`96213b4`、`4e23f91`。
- 删除时发现服务器废纸篓、保存新 UID、批量结果提示：`c02e3e4`。
- 严格检查 LIST 完成状态：`b539053`。
- 云编译与安全测试说明：个人分支自己的工作流和 `TESTING.md`。

保留已有提交及原分支，不重写已分享历史。将来新增修复按一个问题一组提交，
不要和版本号升级、上游同步混在一起。若上游已经修好同一问题，应比较行为和
测试后移除多余的个人改动；不能只凭提交标题自动删补丁。

分支隔离减少污染和重复改动，**不能保证零冲突**。尤其修改同一函数、接口或
依赖时，需要人工合并和回归测试。不要把核心逻辑硬拆成插件，以免增加维护成本。

## 构建限制

上游 `.github/workflows/ci.yml` 保持原样；个人构建放在独立的
`.github/workflows/personal-build.yml`。测试包不继承上游 OAuth 客户端参数，
不要未经验证覆盖日常 OAuth 版安装。详见 `TESTING.md`。
