# 自建分享服务器

[English](../server.md)

分享 ▸ 在线默认把文件发到 `collab.craftapps.dev`。换成自己的服务器后，文件留在你的机器上。服务器程序是 `figcraft-server`：没有账号，**链接和服务器密钥就是权限**。磁盘上的文件不加密，能读数据目录的人能读每一份设计。

下面的地址换成你自己的。密钥只给信任的人。

## 先在这台 Windows 上试

在仓库根目录：

```powershell
cargo build --release -p figcraft-server
New-Item -ItemType Directory -Force D:\figcraft-data | Out-Null
.\target\release\figcraft-server.exe --listen 127.0.0.1:47900 --data D:\figcraft-data
```

这个窗口留着。另开一个 PowerShell，打印发布地址（里面有密钥）：

```powershell
.\target\release\figcraft-server.exe --data D:\figcraft-data key
```

会打出类似 `ws://127.0.0.1:47900/?key=……`。

在 FigCraft 里：

1. **偏好设置 → 在线分享所用的服务器** 写成 `ws://127.0.0.1:47900/`。这样「开始分享」和「用代码加入」不再去 `collab.craftapps.dev`。加入的每一台电脑都要写成同一个地址。
2. **文件 → 发布到服务器**（或 **分享 → 高级 → 发布到其他服务器…**），把上面带 `?key=` 的整段地址贴进去。

发布完成后，分享面板里有编辑链接和查看链接。对方用链接加入，或用加入码（对方的「在线分享所用的服务器」必须是同一台）。

只想关掉默认服务器、先不分享：把「在线分享所用的服务器」清空。在线分享关闭，只剩本地网络。

`127.0.0.1` 只有这台电脑能连。同一局域网里的人要连进来时，把监听改成：

```powershell
.\target\release\figcraft-server.exe --listen 0.0.0.0:47900 --data D:\figcraft-data
```

地址用这台电脑的局域网 IP，例如 `ws://192.168.1.20:47900/?key=……`。Windows 防火墙要放行 47900。不要把 47900 直接暴露到公网：外面的人请走下面的 HTTPS。

## 用 Docker

镜像在 `packaging/server/Dockerfile`。在 Docker Desktop（Linux 容器）里构建，第一次会在容器里编译 Rust，比较久。在仓库根目录：

```powershell
docker build -f packaging/server/Dockerfile -t figcraft-server .
New-Item -ItemType Directory -Force D:\figcraft-server | Out-Null
$toml = "listen = `"0.0.0.0:47900`"`ndata = `"/data`"`npublic_url = `"ws://127.0.0.1:47900/`"`n"
[System.IO.File]::WriteAllText("D:\figcraft-server\server.toml", $toml, (New-Object System.Text.UTF8Encoding $false))
docker run -d --name figcraft-server --restart unless-stopped -p 127.0.0.1:47900:47900 -v figcraft-data:/data -v D:\figcraft-server\server.toml:/etc/figcraft-server/server.toml:ro figcraft-server
docker exec figcraft-server /usr/local/bin/figcraft-server --config /etc/figcraft-server/server.toml key
```

`public_url` 必须写成你在 FigCraft 里能连上的地址。容器自己的网卡地址从 Windows 上连不到。上面这个只给本机：偏好设置里的服务器写成 `ws://127.0.0.1:47900/`，发布时贴 `key` 打出来的整段地址。

文件在 Docker 卷 `figcraft-data` 里。`docker stop figcraft-server` 会先保存再停。升级时重新 `docker build`，然后 `docker rm -f figcraft-server`，再用上面的 `docker run`。卷留着，文件还在。

镜像里没有 shell。管理命令直接执行那个二进制，不要 `docker exec` 进 bash。

同一局域网要把端口改成 `-p 47900:47900`，并把 `public_url` 改成 `ws://这台电脑的局域网IP:47900/`。给公网用时仍然要在前面加 TLS 反代，`public_url` 写成 `wss://你的域名/`，端口只发布到 `127.0.0.1`。

## 给外网的人用

一台 Linux 服务器（1 核、1 GB 内存够一个小团队），一个指向它的域名，前面用 Caddy 或 nginx 做 TLS。FigCraft 认的是 `wss://`，证书用 Let's Encrypt 即可。完整步骤、systemd、Docker 和备份在英文 [server.md](../server.md) 的 Deploy on a Linux server。最小配置是：

```toml
listen = "127.0.0.1:47900"
data = "/var/lib/figcraft-server"
public_url = "wss://figcraft.example.com/"
trusted_proxy = "127.0.0.1"
```

服务器只听本机，443 由反代转到它。防火墙只开 22、80、443。

在服务器上打印密钥：

```sh
sudo -u figcraft figcraft-server --config /etc/figcraft-server/server.toml key
```

每台 FigCraft 的「在线分享所用的服务器」写成 `wss://figcraft.example.com/`。发布时贴 `wss://figcraft.example.com/?key=……`。

健康检查：`https://figcraft.example.com/healthz` 应返回 `ok`。

## 私钥服务器和对外开放

默认是私钥服务器：没有密钥就不能发布。这是给自己团队用的方式。

`open_publish = true` 时，任何人只要能连上服务器就能发布，不用密钥，文件用一次性的所有者令牌管理。那是对外开放的托管，要配联系方式、条款和限额。团队内部不要开。

## 停掉和备份

在跑着的窗口里 Ctrl+C 会先保存再退出。硬杀掉也不会丢掉已经确认的修改。

备份时先停服务器，再复制整个数据目录（Windows 上是 `D:\figcraft-data`）。里面有全部文件和 **服务器密钥**，按服务器本身一样保管。

## 内部版本：固定服务器，并且要登录

给内部用的桌面程序在编译时锁住分享，运行时再去 ERP 登录。地址和密钥都来自环境变量，不要写进仓库。

编译桌面程序（PowerShell）：

```powershell
$env:FIGCRAFT_INTERNAL = "1"
$env:FIGCRAFT_SHARE_SERVER = "ws://127.0.0.1:47900/"
cargo build -p figcraft --release
```

`FIGCRAFT_SHARE_SERVER` 不要带密钥。密钥仍在分享窗口里填写，只发给这一个地址。

这样编译出来的程序：

- 不能改「在线分享所用的服务器」
- 不能在本地网络分享，也不能加入别人的本地分享
- 不能把文件发到别的服务器
- 开始分享前要先登录

运行桌面程序前再设置 ERP（密码和签名密钥不要提交）：

```powershell
$env:FIGCRAFT_ERP_URL = "https://example.invalid"
$env:FIGCRAFT_ERP_APPID = "1001"
$env:FIGCRAFT_ERP_SECRET = "<签名密钥>"
$env:FIGCRAFT_ERP_CLIENT = "EnterpriseCenterWEB"
```

登录请求和现有 ERP 前端一样：`POST /open/Login/userLogin`，正文是 `{ "data": "<json>" }`，签名是把 `appid`、`data`、`secret`、`timestamp` 排序后做 MD5。刷新用 `POST /open/Login/refreshToken`。

服务器要核对这个登录时，再设置 Redis。不设的话，服务器行为和原来一样，不检查登录。ThinkPHP 把用户存在 `user_token:{accessToken}`，值是 PHP `serialize` 的数组。分享链接上的名字用这里的 `nickname`，没有再用 `username`，不采用客户端自己填的名字。

```powershell
$env:FIGCRAFT_REDIS_URL = "redis://:密码@127.0.0.1:6379/0"
$env:FIGCRAFT_ERP_APPID = "1001"
```

`FIGCRAFT_ERP_APPID` 要和登录时写入的 `from_appid` 一致。对不上，或者 Redis 里没有这个 token，发布和新建分享链接都会被拒绝。已经发出去的链接，token 过期之后仍然能用。看的人不用登录。

Docker 里没有 shell。Redis 用另一个容器，把上面两个变量传给 `figcraft-server` 进程即可。镜像本身不用为这个重新编译。
