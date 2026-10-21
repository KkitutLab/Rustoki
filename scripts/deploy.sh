#!/bin/bash
# Tag-driven deploy for Rustoki.
#
#   scripts/deploy.sh [TAG] [--check] [--force]
#
# TAG가 없으면 원격의 가장 최신 X.Y.Z 태그를 대상으로 한다.
# --check 모드에서는 이미 최신 태그에 있으면 아무 것도 하지 않고 종료한다 (cron용).
# 기본 동작: fetch tags -> 태그 checkout -> cargo build --release -> tmux 세션 재시작.
# 태그와 Cargo.toml 버전이 다르면 중단한다 (--force로 무시 가능).
#
# cron 예시 (2분마다 폴링):
#   */2 * * * * /home/kkitut/DEVS/Rustoki/scripts/deploy.sh --check >>/home/kkitut/DEVS/Rustoki/deploy.log 2>&1
set -u
export PATH="$HOME/.cargo/bin:/usr/local/cargo/bin:/usr/local/bin:/usr/bin:/bin:$PATH"

SESSION_NAME="${SESSION_NAME:-rt}"
TAG=""
CHECK=0
FORCE=0
for arg in "$@"; do
    case "$arg" in
        --check) CHECK=1 ;;
        --force) FORCE=1 ;;
        -*) echo "unknown option: $arg" >&2; exit 2 ;;
        *) TAG="$arg" ;;
    esac
done

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_DIR" || exit 1

log() { echo "$(date '+%Y-%m-%d %H:%M:%S') [deploy] $1"; }

log "repo: $REPO_DIR"

if ! git fetch origin --tags --prune 2>&1 | tail -3; then
    log "ERROR: git fetch failed"
    exit 1
fi

if [ -z "$TAG" ]; then
    TAG="$(git tag --list | grep -E '^[0-9]+\.[0-9]+\.[0-9]+$' | sort -V | tail -1)"
fi
if [ -z "$TAG" ]; then
    log "no X.Y.Z tag found, nothing to deploy"
    exit 0
fi
if ! git rev-parse -q --verify "refs/tags/$TAG" >/dev/null; then
    log "ERROR: tag '$TAG' not found"
    exit 1
fi
TARGET_SHA="$(git rev-list -n 1 "$TAG")"
HEAD_SHA="$(git rev-parse HEAD)"
log "target tag: $TAG ($TARGET_SHA), HEAD: $HEAD_SHA"

if [ "$HEAD_SHA" = "$TARGET_SHA" ]; then
    log "already on $TAG, nothing to do"
    exit 0
fi
if [ "$CHECK" = 1 ]; then
    # --check인데 아직 최신 태그가 아니면 배포 진행 (cron 폴링의 정상 케이스).
    log "new tag detected: $TAG (current HEAD: $HEAD_SHA)"
fi

if [ "$FORCE" = 0 ]; then
    TOML_VER="$(git show "$TAG:Cargo.toml" 2>/dev/null | grep -m1 '^version' | sed 's/.*"\(.*\)".*/\1/')"
    if [ -z "$TOML_VER" ]; then
        log "ERROR: cannot read version from $TAG:Cargo.toml"
        exit 1
    fi
    if [ "$TOML_VER" != "$TAG" ]; then
        log "ERROR: tag $TAG != Cargo.toml version $TOML_VER (use --force to override)"
        exit 1
    fi
    log "version check ok: tag == Cargo.toml ($TOML_VER)"
fi

log "checkout $TAG"
git checkout "$TAG" || { log "ERROR: checkout failed"; exit 1; }

log "cargo build --release"
if ! cargo build --release; then
    log "ERROR: build failed, keeping current binary running"
    exit 1
fi

BIN="$REPO_DIR/target/release/Rustoki"
log "binary rebuilt: $(ls -l --time-style=long-iso "$BIN" | awk '{print $6, $7}')"

if command -v tmux >/dev/null 2>&1; then
    log "restart tmux session: $SESSION_NAME"
    tmux kill-session -t "$SESSION_NAME" 2>/dev/null
    tmux new-session -d -s "$SESSION_NAME" -c "$REPO_DIR" "./target/release/Rustoki"
    log "tmux session $SESSION_NAME restarted"
else
    log "tmux not found, start $BIN manually"
fi

log "deployed $TAG done"
