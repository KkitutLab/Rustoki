#!/bin/bash
# Tag-driven deploy for Rustoki.
#
#   scripts/deploy.sh [TAG] [--check] [--force]
#
# TAG가 없으면 원격의 가장 최신 X.Y.Z 태그를 대상으로 한다 (자동 모드).
# --check 모드에서는 이미 최신 태그에 있으면 아무 것도 하지 않고 종료한다 (cron용).
# 기본 동작: fetch tags -> 태그 checkout -> cargo build --release -> tmux 세션 재시작.
# - 빌드 내내 기존 봇은 계속 돌아가고, 빌드가 끝나야만 재시작한다.
# - 빌드 중에 더 새 태그가 들어오면(자동 모드) 현재 빌드를 중단하고 새 태그로 다시 빌드한다.
# - 진행 상황은 .deploy-state 파일에 기록되며, 실행 중인 봇이 읽어서
#   Updating / Rebooting 로그를 남긴다.
# 태그와 Cargo.toml 버전이 다르면 중단한다 (--force로 무시 가능).
#
# cron 예시 (2분마다 폴링):
#   */2 * * * * /home/kkitut/DEVS/Rustoki/scripts/deploy.sh --check >>/home/kkitut/DEVS/Rustoki/deploy.log 2>&1
set -u
# 작업 제어 켜기: 백그라운드 빌드가 독자 프로세스 그룹을 갖게 해서
# 중단 시 cargo+rustc를 그룹킬할 수 있게 한다.
set -m
export PATH="$HOME/.cargo/bin:/usr/local/cargo/bin:/usr/local/bin:/usr/bin:/bin:$PATH"

SESSION_NAME="${SESSION_NAME:-rt}"
BIN_NAME="${BIN_NAME:-Rustoki}"
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
PINNED="$TAG"

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_DIR" || exit 1
STATE_FILE="$REPO_DIR/.deploy-state"
LOCK_FILE="$REPO_DIR/.deploy.lock"

log() { echo "$(date '+%Y-%m-%d %H:%M:%S') [deploy] $1"; }
set_state() { echo "$1" > "$STATE_FILE"; }
clear_state() { rm -f "$STATE_FILE"; }

latest_tag() {
    git tag --list | grep -E '^[0-9]+\.[0-9]+\.[0-9]+$' | sort -V | tail -1
}

version_ok() {
    local ver
    ver="$(git show "$1:Cargo.toml" 2>/dev/null | grep -m1 '^version' | sed 's/.*"\(.*\)".*/\1/')"
    [ -n "$ver" ] && [ "$ver" = "$1" ]
}

start_build() {
    cargo build --release &
    BUILD_PID=$!
}

stop_build() {
    kill -TERM -"$BUILD_PID" 2>/dev/null
    wait "$BUILD_PID" 2>/dev/null
}

log "repo: $REPO_DIR"

if ! git fetch origin --tags --prune 2>&1 | tail -3; then
    log "ERROR: git fetch failed"
    exit 1
fi

TARGET="$PINNED"
if [ -z "$TARGET" ]; then
    TARGET="$(latest_tag)"
fi
if [ -z "$TARGET" ]; then
    log "no X.Y.Z tag found, nothing to deploy"
    exit 0
fi
if ! git rev-parse -q --verify "refs/tags/$TARGET" >/dev/null; then
    log "ERROR: tag '$TARGET' not found"
    exit 1
fi
TARGET_SHA="$(git rev-list -n 1 "$TARGET")"
HEAD_SHA="$(git rev-parse HEAD)"
log "target tag: $TARGET ($TARGET_SHA), HEAD: $HEAD_SHA"

if [ "$HEAD_SHA" = "$TARGET_SHA" ]; then
    log "already on $TARGET, nothing to do"
    exit 0
fi
if [ "$CHECK" = 1 ]; then
    log "new tag detected: $TARGET (current HEAD: $HEAD_SHA)"
fi

exec 9>"$LOCK_FILE"
if ! flock -n 9; then
    log "another deploy in progress, exit (it will pick up new tags itself)"
    exit 0
fi

if [ "$FORCE" = 0 ] && ! version_ok "$TARGET"; then
    log "ERROR: tag $TARGET != Cargo.toml version (use --force to override)"
    exit 1
fi
log "version check ok: tag == Cargo.toml"

log "checkout $TARGET"
git checkout "$TARGET" || { log "ERROR: checkout failed"; exit 1; }

set_state "updating $TARGET"
log "build $TARGET (old bot keeps running)"
start_build

while kill -0 "$BUILD_PID" 2>/dev/null; do
    sleep 20
    git fetch origin --tags --prune >/dev/null 2>&1
    if [ -z "$PINNED" ]; then
        LATEST="$(latest_tag)"
        if [ -n "$LATEST" ] && [ "$LATEST" != "$TARGET" ]; then
            if version_ok "$LATEST"; then
                log "newer tag $LATEST arrived during build, restart build for it"
                stop_build
                TARGET="$LATEST"
                log "checkout $TARGET"
                git checkout "$TARGET" || { log "ERROR: checkout failed"; clear_state; exit 1; }
                set_state "updating $TARGET"
                start_build
            else
                log "newer tag $LATEST version mismatch, keep building $TARGET"
            fi
        fi
    fi
done

if ! wait "$BUILD_PID"; then
    log "ERROR: build failed, keeping current binary running"
    clear_state
    exit 1
fi

BIN="$REPO_DIR/target/release/$BIN_NAME"
log "binary rebuilt: $(ls -l --time-style=long-iso "$BIN" | awk '{print $6, $7}')"

set_state "rebooting $TARGET"
log "rebooting into $TARGET (give the running bot a moment to announce)"
sleep 5

if command -v tmux >/dev/null 2>&1; then
    log "restart tmux session: $SESSION_NAME"
    clear_state
    tmux kill-session -t "$SESSION_NAME" 2>/dev/null
    tmux new-session -d -s "$SESSION_NAME" -c "$REPO_DIR" "./target/release/$BIN_NAME"
    log "tmux session $SESSION_NAME restarted"
else
    clear_state
    log "tmux not found, start $BIN manually"
fi

log "deployed $TARGET done"
