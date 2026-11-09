# 单分支补丁版

只维护 `master`。程序源码保持上游内容；个人修复和回归测试集中在
`pebble-fixes.patch`，云构建临时应用，不把展开后的改动提交回源码。

`manifest.json` 记录原始上游基线、修复来源提交、文件列表和 SHA256。
补丁包含 IMAP 同步/断连恢复、废纸篓发现、删除 UID 跟踪及界面反馈修复。
补丁还修复 OAuth 本机配置查找：优先读取可执行文件所在目录的 `.env`，
避免 Windows 开机启动或文件关联从其他工作目录启动时误用占位应用编号。
进程环境变量仍优先于文件配置；原开发目录回退保留。回归测试也封装在补丁中。
备份标签 `backup/pre-patch-layout-20261108` 保留旧修复提交，它不是维护分支。

## 构建

推送 `master` 自动运行 **Patched Build**，或在 Actions 中手动运行它。
先校验补丁 SHA256 和可应用性，再应用、格式检查、Clippy、测试及三平台打包。
安装包名称带 `pebble-patched-*`。上游原有的 **CI** 仍保留原样，检查的是
未打补丁的上游源码；**带修复的包请认准 Patched Build**，不要混用原 CI/Release。

本地构建（仓库根目录）：

```powershell
node scripts/pebble-patch.mjs --check
node scripts/pebble-patch.mjs --apply
pnpm test
pnpm build:windows
node scripts/pebble-patch.mjs --reverse
git status
```

应用后源码会显示为工作区修改，这是正常的。不要提交这些展开的修改；只提交
补丁、清单和必要的独立构建配置。撤销只反向应用补丁，不使用强制 reset/clean。

## 同步上游

先反向应用补丁，并确认工作区干净，再执行：

```powershell
git fetch --no-tags upstream master
git merge --no-edit upstream/master
node scripts/pebble-patch.mjs --check
git push origin master
```

如果新克隆没有 upstream：`git remote add upstream https://github.com/QingJ01/Pebble.git`。
也可以在 GitHub 使用 Sync fork 更新上游，再在本地拉取。**不能保证零冲突**：
上游源码合并通常不再碰个人修复，但补丁可能过时，检查失败时必须适配补丁后
再构建。不要用 `--reject` 或忽略失败继续发布。

### 每周自动同步

**Weekly Upstream Sync** 每周一北京时间 **09:17**（UTC 01:17）检查上游。
任务运行在 GitHub，不需要本机开机；计划任务可能因 GitHub 队列延迟。

- 没有新提交：不改分支、不编译。
- 有新提交：先在临时 runner 合并并验证补丁；都通过才普通 push 到 `master`，
  然后直接调用 Patched Build，以这次同步的精确提交编译三平台。
- 合并冲突、补丁失效：停止，远端 `master` 不会被本次同步修改。编译或测试失败：
  Actions 显示失败，不发布 Release、不自动安装到电脑。
- 打包工具下载遇到 HTTP 429/5xx 时最多尝试 3 次；不会重试或忽略代码、测试错误。
- GitHub 的 `GITHUB_TOKEN` 推送不会再次触发 push 工作流，因此这里显式调用
  可复用构建，不依赖 push 连锁触发，也不需要另存个人访问令牌。
- 在 Actions 手动运行同一任务可立即检查；`rebuild` 默认关闭，仅在修复构建失败
  后需要重编译或验证构建链路时勾选。构建产物显示在该同步运行的 Artifacts 中。
- 失败通知使用你的 GitHub Actions 通知设置。公开仓库连续约 60 天没有活动时，
  GitHub 可能停用定时任务；发现停用后需在 Actions 重新启用。

该同步只维护 `master`，不会创建新分支、强推、推送上游标签或改动备份标签。

上游已吸收某项修复时，核对行为与测试后删掉对应补丁段、重新生成清单和校验值，
再跑完整回归。保留 `git diff --binary --full-index` 格式，避免手工改补丁行号。

## 安装安全

Fork 不继承上游 Google/Microsoft OAuth 参数。没有配置相应参数的构建不要覆盖
日常 OAuth 版。禁止用真实邮箱测试破坏性删除，测试只用 loopback 或专用测试邮件。
真实日志、账号数据库及凭据不进入补丁、不上传仓库。
