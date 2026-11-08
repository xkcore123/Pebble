# 单分支补丁版

只维护 `master`。程序源码保持上游内容；个人修复和回归测试集中在
`pebble-fixes.patch`，云构建临时应用，不把展开后的改动提交回源码。

`manifest.json` 记录原始上游基线、修复来源提交、文件列表和 SHA256。
补丁包含 IMAP 同步/断连恢复、废纸篓发现、删除 UID 跟踪及界面反馈修复。
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
再构建。不要用 `--reject` 或忽略失败继续发布。没有设置定时同步。

上游已吸收某项修复时，核对行为与测试后删掉对应补丁段、重新生成清单和校验值，
再跑完整回归。保留 `git diff --binary --full-index` 格式，避免手工改补丁行号。

## 安装安全

Fork 不继承上游 Google/Microsoft OAuth 参数。没有配置相应参数的构建不要覆盖
日常 OAuth 版。禁止用真实邮箱测试破坏性删除，测试只用 loopback 或专用测试邮件。
真实日志、账号数据库及凭据不进入补丁、不上传仓库。
