# AkaNetease 后端

独立 Rust Cargo 工程，不依赖前端、CloudMusicPlayer 或 ncm-api。计划见根目录 `BACKEND_PLAN.md`，对接格式见 `BACKEND_API.md`。

## 构建与验证

需要 Rust 1.89+（2024 edition）工具链、pkg-config 和 GStreamer 1.x 开发库。音频运行时需要 playbin、WAV/MP3/FLAC 解码插件及系统音频输出；测试使用自行生成的 WAV 和 fakesink，不会发声。

```sh
cd backend
cargo fmt --all -- --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --no-default-features
```

`--no-default-features` 不构建 GStreamer，适合只开发服务接口和核心逻辑。HTTP 测试绑定本机临时端口，无需外部网络。Cargo.lock 固定本次依赖解析结果。

## 调试入口

```sh
cargo run --locked -- login
cargo run --locked -- session
cargo run --locked -- playlists 0 20
cargo run --locked -- liked
cargo run --locked -- daily
cargo run --locked -- search 'Beyond'
cargo run --locked -- track 347230
cargo run --locked -- artist-songs 6452
cargo run --locked -- artist-albums 6452 0 20
cargo run --locked -- playlist 3778678 20
cargo run --locked -- lyrics 347230
cargo run --locked -- stream 347230
cargo run --locked -- play /absolute/path/to/music.wav
```

查询输出 JSON；错误输出到 stderr 并返回非零退出码。stream 仅解析地址，不保证当前匿名会话具有播放权限，也不绕过服务端限制；调试输出可能含短期播放 URL。网络请求有连接/总时限和响应体大小上限；普通查询不自动重试，扫码入口会在网络暂时失败时按轮询间隔重试。

构建后可手动运行 `python scripts/live_smoke.py`，通过实际 CLI 只读检查在线接口；脚本仅输出计数与状态，不保存歌曲、歌词或音频。此检查需要网络，不属于日常离线测试；播放地址不可用会明确标注，不能据此认为在线播放已验证。

login 在终端渲染二维码并轮询等待扫码，Ctrl+C 取消。成功后进入交互模式，可输入 status、playlists、liked、daily、stream <id>、logout、quit。内存会话可在该进程内使用；quit 退出进程，保留成功存入钥匙串的会话。`login --memory-only` 使用临时会话，完全不读取或修改系统钥匙串。

跨进程恢复需要 `secret-tool`（Arch 的 libsecret；Debian/Ubuntu 的 libsecret-tools）、会话 D-Bus 和可用的 Secret Service（如 GNOME Keyring 或兼容提供者）。使用固定应用属性保存 Cookie，不将凭据放入文件、命令参数或日志；可能需要在桌面上解锁钥匙串。凭据操作超时为 10 秒；保存不可用时返回 memory_only，如果旧凭据无法删除则返回 cleanup_required。看到 cleanup_required 应恢复钥匙串服务并重试 logout，避免旧凭据在下次启动恢复。

session、playlists、liked、daily 读取和校验已保存会话；新 login 会先清除旧内存会话和保存的凭据。logout 先尝试恢复当前账号，再清除本地凭据并请求注销；网络不通时仍执行本地清理，输出分别标明清理和注销结果。当前每个应用进程应只创建一个 AccountService；多进程同时修改同一个钥匙串条目尚未提供跨进程协调。

`cargo build --locked` 后运行 `python scripts/live_auth_smoke.py` 可验证实际二维码创建、等待扫码与本地取消，全程不接触钥匙串，不显示二维码内容。当前在线证据覆盖此流程；真实扫码授权、真实钥匙串往返和个人数据接口仍需账号验收，离线测试使用合成 Cookie 和临时模拟服务。

play 是单音源诊断入口，结束或错误时退出，Ctrl+C 可中止；不是完整的队列播放器。播放器可在库 API 中暂停、恢复和定位，宿主需定期消费事件。M3 新增 PlayerHandle 服务负责队列连续播放，Tauri 与 MPRIS 使用同一服务；CLI play 继续保留为独立诊断入口。M4 已补上歌词缓存和发行脚本；音频缓存/下载另行排期。

## M3 桌面宿主与系统媒体控制

`backend/` 是可单独构建的核心/CLI，`backend/desktop/` 是独立 Tauri 2 crate，各自锁定 Cargo.lock。宿主只通过配置引用前端产物，不导入前端源码；前端仍可独立运行 Vite、使用 mock 并重构 UI。MPRIS 为核心可选 feature，宿主默认启用。

Linux 桌面构建还需要 GTK3、WebKitGTK 4.1、librsvg、对应开发头文件和链接库；系统媒体控制需要会话 D-Bus。GStreamer 需要相应网络源、解码及音频输出插件。宿主初始化音频失败时退出；MPRIS 初始化失败只影响系统媒体控制，不影响应用内播放。

```sh
# 在 backend 下执行
cargo test --locked --features mpris
# 该项在普通测试中忽略，必须用独立会话显式运行
dbus-run-session -- cargo test --locked --features mpris --test mpris -- --ignored
cargo clippy --locked --all-targets --features mpris -- -D warnings
cargo fmt --all --manifest-path desktop/Cargo.toml -- --check
cargo test --locked --manifest-path desktop/Cargo.toml
cargo clippy --locked --manifest-path desktop/Cargo.toml --all-targets -- -D warnings
cargo build --locked --manifest-path desktop/Cargo.toml
```

本地 HTTP 测试需要绑定临时端口；Tauri MockRuntime 会初始化应用目录；受限沙箱可能需要相应权限。测试使用临时自建 WAV 和无声 sink，不访问真实账号或钥匙串。独立 D-Bus 测试通过代理调用同一个真实 GStreamer 播放服务，不影响已有桌面媒体会话。

前端 Agent 实现并启动 `http://localhost:5173` 后，可在 backend 运行 `cargo run --locked --manifest-path desktop/Cargo.toml` 启动宿主；前端现已交付真实界面及 dist，基础 WebView 联调已通过。发布构建使用 `cargo build --locked --release --manifest-path desktop/Cargo.toml --features custom-protocol`，需要前端先生成 frontend/dist。不会自动代建前端或生成占位页面；Linux tar 发行脚本见 packaging/README.md。

前端接入使用根目录 BACKEND_API v1.2：先订阅 player-state，再获取快照；以 sequence 去重，以 queueRevision 和 selectionId 防止过期编辑。账号恢复由前端显式调用 session_restore。登录成功不等待钥匙串保存，后续 session-state 更新存储状态；正常退出会等待凭据操作收尾。歌手热门单曲通过 music_artist_songs 获取最多 50 首；专辑列表通过 music_artist_albums 分页获取 Page<AlbumSummary>，具体字段与分页语义见根契约。基础真实 WebView 联调已通过；真实账号音源、钥匙串及硬件媒体键仍需验收，不将 MockRuntime 和合成音频测试视为完整产品验收。

扫码诊断：桌面 `backend.jsonl` 的 auth 事件包含 phase、outcome、elapsedMs、成功轮询状态 200/800..803 的 qrCode，以及失败时受范围限制的 serviceCode。可区分 create/poll/cookie/profile/credential_clear/credential_save/restore 阶段；不记录用户身份、二维码 key/URL、Cookie 或原始错误消息。配置路径可通过 `aka-backend paths` 查询，日志位于 stateDir。

## M4 独立后端能力

歌单播放先通过 `player_replace` 播放已显示曲目，后台分页加载时通过 `player_expand({tracks,revision})` 扩充队列，保留当前歌曲、音源和进度，不重新解析或打断暂停。每次更新使用最新返回的 queueRevision；用户替换/删除队列时，旧更新返回 stale_operation。单队列仍限制 10000 首，未返回元数据的歌曲无法入队，契约见 BACKEND_API v1.2。

应用配置位于 `${XDG_CONFIG_HOME:-~/.config}/akanetease/config.json`，播放状态和单实例锁位于 `${XDG_STATE_HOME:-~/.local/state}/akanetease`，公开歌词缓存和容量限制位于 `${XDG_CACHE_HOME:-~/.cache}/akanetease`。配置包含 `proxy`（`system`、`direct` 或不含凭据的 HTTP origin）、`cacheLimitBytes` 和 `restoreQueue`。`config_set` 写入后需重启宿主才应用新代理和缓存实例。

后端启动会尝试恢复上次队列，但保持停止，不会在启动时自动请求短期音源。退出和定时检查会原子保存状态；损坏状态会保留现场并进入 degraded 状态，避免覆盖用户数据。公开歌词按歌曲 ID 缓存 7 天并受容量限制，缓存清理不会触碰其他应用文件。

```sh
cargo run --locked -- config
cargo run --locked -- paths
cargo run --locked -- cache-stats
cargo run --locked -- cache-clear
cargo run --locked -- config-set /path/to/config.json
python scripts/doctor.py --build
python scripts/package_linux.py --check
```

诊断日志位于 XDG state 目录的 `backend.jsonl`，只包含时间戳和固定事件名。打包脚本需要前端先提供真实 `frontend/dist/index.html`；缺少时会返回错误并不生成发布包。

默认配置（无配置文件时只读取此默认值，不自动写入）：

```json
{"version":1,"cacheLimitBytes":33554432,"proxy":{"mode":"system"},"restoreQueue":true}
```

可选 `proxy:{"mode":"direct"}` 或 `proxy:{"mode":"http","url":"http://127.0.0.1:7890"}`。暂不支持代理认证、SOCKS 或前端封面请求代理；system 模式由各网络库读取自己的环境配置。缓存默认 32 MiB，范围 0..1 GiB，0 禁用；每条歌词最多 2 MiB，按最旧写入时间淘汰。CLI cache-stats/cache-clear 会取得宿主同一把锁，请先退出桌面进程再使用；在宿主内调用 IPC cache_* 可即时操作。

状态文件保存间隔为 5 秒，正常退出先保存再停止播放器，SIGKILL 无法保存。恢复随机模式会重新洗牌但保留当前歌曲索引；恢复进度在点击播放后才执行。restoreQueue=false 同时停用状态读取和保存，不删除旧状态。状态损坏时本进程禁止覆盖，退出后备份/移走 player.json 再重启可恢复自动保存。错误配置会阻止启动，可以用 config-set 写入完整正确配置修复。

`backend_status` 可查询 MPRIS 启动注册、持久化降级、队列是否恢复和缓存初始化结果；日志约 1 MiB 后轮转为 backend.jsonl.1，只保留当前及上一个文件。此状态并非持续的 D-Bus/磁盘健康监控。

## 已交付 UI 的集成验证

前端 `268374d` 已交付真实 dist。现在可直接构建嵌入界面的程序：

```sh
cargo run --locked --manifest-path desktop/Cargo.toml --features custom-protocol
python scripts/webview_check.py
python scripts/package_linux.py --offline --output dist/desktop-preview
python scripts/package_smoke.py /absolute/path/to/extracted/usr/bin/akanetease-desktop
```

webview_check 会临时打开真实 WebKit 窗口，使用 ephemeral 账号、合成 WAV 无声播放和歌词缓存验证 UI/IPC，不操作真实钥匙串。测试二进制单独以 webview-check feature 编译，未包含在发行包。package_smoke 启动真实发行二进制，在临时 XDG 和禁止服务自动激活的私有 D-Bus 中检查 MPRIS 与退出保存，secret-tool 在该测试中刻意不可用。

联调结果和前端待修复项见根目录 INTEGRATION_REPORT.md。现有归档位于 dist/desktop-preview；同名输出不会覆盖。后续 UI 或 Rust 更改后需使用新输出目录重新构建归档。
