#!/usr/bin/env bash
# Generate <name>.words.json from <name>.srt with Claude Code in print mode.
#
#   scripts/annotate.sh episode.srt [episode.words.json]
#
# Environment:
#   LISTEN       path to the listen binary (default: listen on PATH)
#   CLAUDE       path to the claude CLI (default: claude on PATH)
#   CLAUDE_ARGS  extra arguments for claude, e.g. "--model sonnet"
#   FORCE=1      overwrite an existing output file
set -euo pipefail

die() { echo "annotate: $*" >&2; exit 1; }

[ $# -ge 1 ] && [ $# -le 2 ] || die "usage: $0 <file.srt> [out.words.json]"
srt=$1
out=${2:-${srt%.srt}.words.json}
listen=${LISTEN:-listen}
claude=${CLAUDE:-claude}
prompt_file="$(cd "$(dirname "$0")" && pwd)/annotate-prompt.md"

[ -f "$srt" ] || die "no such file: $srt"
[ -f "$prompt_file" ] || die "missing prompt file: $prompt_file"
command -v "$listen" >/dev/null || die "listen not found; build it and set LISTEN=path/to/listen"
command -v "$claude" >/dev/null || die "claude CLI not found"
[ ! -e "$out" ] || [ "${FORCE:-}" = 1 ] || die "$out exists; set FORCE=1 to overwrite"

tmp=$(mktemp)
trap 'rm -f "$tmp" "$tmp.raw"' EXIT

# Number sentences exactly the way the app does, then ask Claude.
# shellcheck disable=SC2086 # CLAUDE_ARGS is intentionally word-split
"$listen" cues "$srt" |
  "$claude" -p --output-format text ${CLAUDE_ARGS:-} "$(cat "$prompt_file")" >"$tmp.raw"

# Keep the JSON object even if the reply came wrapped in a code fence.
sed -e '/^[[:space:]]*```/d' "$tmp.raw" >"$tmp"

# Fails on invalid JSON; warns about items that do not match their sentence.
if ! "$listen" check "$srt" "$tmp"; then
  cp "$tmp.raw" "$out.rejected"
  die "the reply was not valid words.json; raw reply saved to $out.rejected"
fi
mv "$tmp" "$out"
echo "wrote $out"
