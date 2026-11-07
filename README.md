# rate

自托管浏览器测速工具。Rust CLI 同源提供 React 页面和测速 API。发布二进制内置前端资源和离线 IP 归属地数据库。

## 启动

```sh
rate --host 0.0.0.0 --port 9000 --base-path /tools/speed
```

打开 `http://localhost:9000/tools/speed/`。默认监听 `127.0.0.1:8080`, 挂载路径为 `/`。使用 `rate --help` 查看全部参数。反向代理转发时保留配置的路径前缀。通过 `--trusted-proxy 127.0.0.1` 等参数指定代理地址后, 服务会从可信代理的 `X-Forwarded-For` 读取客户端 IP。

### HTTP Basic Auth

```sh
rate --basic-auth 'alice:example:password'
```

`--basic-auth` 接收 `用户名:密码`, 按第一个冒号分隔, 密码中的后续冒号原样保留。启用后, 页面、静态资源、测速 API 和健康检查都要求认证。Docker 和服务管理器可通过 `RATE_BASIC_AUTH` 注入同格式的凭据。对外部署时使用 HTTPS 反向代理传输凭据。

### IP 归属地

默认查询内置的 IPinfo MMDB。设置 `--ipinfo-api-key` 或 `RATE_IPINFO_API_KEY` 时, 服务端优先查询 IPinfo Lite, 在线查询失败时回退离线库。数据来源和许可见 [第三方声明](THIRD_PARTY_NOTICES.md)。

## 从源码构建

需要 Rust stable、Node.js、curl、jq 和 shasum。使用 nvm 在项目根目录选择 `.nvmrc` 指定的版本, 并通过环境变量 `IPINFO_DOWNLOAD_TOKEN` 提供可下载 Lite 数据库的 IPinfo access token。

```sh
nvm use
(cd web && npm ci && npm test && npm run build)
bash scripts/fetch-geoip.sh build/geoip/ipinfo_lite.mmdb
cargo test --locked
RATE_GEOIP_DB="$PWD/build/geoip/ipinfo_lite.mmdb" cargo build --locked --release
```

## 发布

GitHub Actions 在新 tag 推送后构建 Windows x64 和 ARM64、Linux x64 和 ARM64、macOS ARM64, 并将五个可执行文件上传到对应的 GitHub Release。下载后的 Linux 或 macOS 文件可用 `chmod +x <文件名>` 添加执行权限。

每次发布构建从 IPinfo 下载最新可用的 Lite MMDB, 用官方 SHA-256 校验后将同一份文件交给五个平台。下载失败或校验始终不一致时, 发布流程会停止。`IPINFO_DOWNLOAD_TOKEN` 用于构建时下载, `RATE_IPINFO_API_KEY` 用于服务运行时的在线查询。

同一次构建把两份 Linux 可执行文件打包成 `linux/amd64` 和 `linux/arm64` 的 Docker Hub 多平台镜像。启用发布前, 在 GitHub 仓库的 Actions 设置中配置以下值。

| 类型 | 名称 | 值 |
| --- | --- | --- |
| Secret | `IPINFO_DOWNLOAD_TOKEN` | 有权下载 [IPinfo Lite 数据库](https://ipinfo.io/developers/ipinfo-lite-database)的 access token |
| Variable | `DOCKERHUB_USERNAME` | Docker Hub 登录账号 |
| Secret | `DOCKERHUB_TOKEN` | 对目标仓库有写权限的 Docker Hub access token |
| Variable | `DOCKERHUB_IMAGE` | 可选的镜像名, 如 `my-org/rate`, 默认为 `<DOCKERHUB_USERNAME>/rate` |

镜像使用 Git tag 作为版本标签, 含 Docker 标签非法字符的 Git tag 使用确定性的 SHA-256 标签。

有多位仓库写入者时, 使用 [GitHub 标签规则集](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/available-rules-for-rulesets#restrict-creations)将发布标签的创建和修改权限限定给发布维护者。标签指向的提交决定发布工作流及构建代码。
