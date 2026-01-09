# MoFA Studio：Nix 环境快速部署

> 适用于 macOS (Arm)；第一次执行会下载 ~2 GB（Rust toolchain + PyTorch/FunASR 等），建议使用稳定网络。

## 1. 安装 Nix

```bash
sh <(curl -L https://nixos.org/nix/install) --daemon
```

安装完成后重新打开终端，或手动执行：

```bash
source /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh
```

## 2. 进入项目

```bash
cd ~/Desktop/code/mofa-org/hackerweek/nix-mofa/mofa-studio
```

（如果放在其它位置，调整路径即可。以下命令均在 `mofa-studio` 仓库根目录执行。）

## 3. 一键启动

```bash
nix --extra-experimental-features 'nix-command flakes' run .
```

该命令会：

1. 自动安装/缓存 Rust toolchain、Node.js、CMake、PortAudio 等编译依赖。
2. 在 `./.nix-mofa` 下 `cargo install` 固定版本的 `dora-cli`。
3. 在 `./.venv-mofa` 创建 Python venv 并 `pip install -e` `libs/dora-common`、`python-nodes/dora-text-segmenter`、`python-nodes/dora-primespeech`。
4. 强制将 `dora-rs` Python 包固定在 `0.3.12`（可用 `MOFA_DORA_RS_VERSION` 覆盖），避免 `message format v0.6` 与 `v0.5` 不兼容报错。
5. `pkill -f dora` → `dora up` → `dora list`/`dora stop --grace-duration 0s …`，保证没有残留的 dataflow。
6. `cargo run --release` 启动 Makepad GUI（`MOFA_AUTO_START=1` 会自动连上 dataflow）。

> **注意**：首次运行耗时较长，期间请保持终端开启；GUI 启动后即可在界面中操作/退出。

### 常用环境变量

| 变量 | 作用 |
| ---- | ---- |
| `MOFA_STUDIO_DIR` | 指定源码路径（默认当前目录 `pwd`） |
| `MOFA_STATE_DIR` | 指定 dora-cli 缓存目录（默认 `./.nix-mofa`） |
| `MOFA_VENV_DIR` | 指定 Python venv 目录（默认 `./.venv-mofa`） |
| `MOFA_DORA_RS_VERSION` | 控制安装的 `dora-rs` 版本（默认 `0.3.12`，用于匹配 `dora-cli`） |
| `MOFA_SKIP_BOOTSTRAP=1` | 跳过 dora/Python 依赖安装，适合二次启动或离线环境 |
| `MOFA_DRY_RUN=1` | 只做环境检查/清理，验证完即退出，不启动 GUI |

示例：在复用现有缓存的情况下，仅启动应用

```bash
MOFA_SKIP_BOOTSTRAP=1 nix --extra-experimental-features 'nix-command flakes' run .
```

若想先验证 `dora`/`dataflow` 流程但不弹出 GUI，可再加 `MOFA_DRY_RUN=1`。

> **提示**：如果之前的 venv 已经安装了 `dora-rs 0.3.13+`，请至少执行一次不带 `MOFA_SKIP_BOOTSTRAP=1` 的 `nix run .`（或手动 `source .venv-mofa/bin/activate && pip install --upgrade --force-reinstall 'dora-rs==0.3.12'`）以确保协议版本一致。

## 4. 开发调试

若需进入带齐依赖的开发 shell（含 Rust/Python/Node/CMake/pkg-config/PortAudio 等），可执行：

```bash
nix --extra-experimental-features 'nix-command flakes' develop .
```

在该 shell 中运行 `cargo`, `python`, `nix run .` 等都已经具备所需工具链。

---

发生异常时，日志会写在 `./out/`（`dora-daemon.txt` / `dora-coordinator.txt`）。如需重新跑首次安装，可删除 `.nix-mofa` 与 `.venv-mofa` 目录后再执行 `nix run .`。
