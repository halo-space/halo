#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

REGISTRY="crates-io"
ALLOW_DIRTY=0
DRY_RUN=0
SKIP_CHECKS=0

usage() {
  cat <<'EOF'
发布 halo workspace 到 crates.io 的标准脚本。

默认发布顺序：
  1) halo-core
  2) halo-rest
  3) halo-micro（lib crate 名为 halo_micro；使用方依赖写 halo_micro = { package = "halo-micro", ... }）

用法：
  ./scripts/publish_crates_io.sh [--dry-run] [--allow-dirty] [--skip-checks] [--registry crates-io]

选项：
  --dry-run       仅做发布演练（不会上传）
  --allow-dirty   允许工作区有未提交改动（不建议，但便于快速试跑）
  --skip-checks   跳过 fmt/clippy/test/package 预检（不建议）
  --registry NAME 指定 registry（默认 crates-io）

前置条件：
  - 已完成：cargo login
  - crates.io 账号已验证邮箱
  - 如环境替换了 crates-io（例如 rsproxy），本脚本会显式使用 --registry crates-io

EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    --allow-dirty)
      ALLOW_DIRTY=1
      shift
      ;;
    --skip-checks)
      SKIP_CHECKS=1
      shift
      ;;
    --registry)
      REGISTRY="${2:-}"
      if [[ -z "${REGISTRY}" ]]; then
        echo "错误：--registry 需要一个参数" >&2
        exit 2
      fi
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "未知参数：$1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

extra_flags=()
if [[ "${ALLOW_DIRTY}" -eq 1 ]]; then
  extra_flags+=(--allow-dirty)
fi
publish_flags=("${extra_flags[@]}")
if [[ "${DRY_RUN}" -eq 1 ]]; then
  publish_flags+=(--dry-run)
fi

packages=(
  "halo-core"
  "halo-rest"
  "halo-micro"
)

echo "==> registry: ${REGISTRY}"
echo "==> dry-run:  ${DRY_RUN}"
echo "==> allow-dirty: ${ALLOW_DIRTY}"
echo "==> packages: ${packages[*]}"

if [[ "${SKIP_CHECKS}" -eq 0 ]]; then
  echo "==> 预检：fmt/clippy/test"
  cargo fmt
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace

  echo "==> 预检：cargo package（仅打包检查，提前发现发布问题）"
  for pkg in "${packages[@]}"; do
    cargo package -p "${pkg}" "${extra_flags[@]}"
  done
fi

echo "==> 发布到 crates.io"
for pkg in "${packages[@]}"; do
  echo "==> cargo publish -p ${pkg}"
  cargo publish -p "${pkg}" --registry "${REGISTRY}" "${publish_flags[@]}"
done

echo "==> 完成"


