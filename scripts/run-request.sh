#!/usr/bin/env bash
# Usage: run-request.sh <file> <line>
# Extracts the HTTP request block containing the given line and executes it with curl.
# Writes response to <file>.response.http (open this in a split pane — it auto-reloads).

FILE="$1"
LINE="$2"

if [ -z "$FILE" ] || [ -z "$LINE" ]; then
  echo "Usage: run-request.sh <file> <line>"
  exit 1
fi

RESPONSE_DIR="/tmp/rest-client-zed"
mkdir -p "$RESPONSE_DIR"
RESPONSE_FILE="${RESPONSE_DIR}/response.http"

block_start=1
block_end=$(wc -l < "$FILE")

while IFS= read -r num; do
  if [ "$num" -lt "$LINE" ]; then
    block_start=$((num + 1))
  fi
done < <(grep -n '^###' "$FILE" | cut -d: -f1)

while IFS= read -r num; do
  if [ "$num" -ge "$LINE" ]; then
    block_end=$((num - 1))
    break
  fi
done < <(grep -n '^###' "$FILE" | cut -d: -f1)

block=$(sed -n "${block_start},${block_end}p" "$FILE")

method=""
url=""
headers=()
body=""
phase="pre"

var_names=()
var_values=()
while IFS= read -r line; do
  if [[ "$line" =~ ^@([A-Za-z_][A-Za-z0-9_]*)\ *=\ *(.*) ]]; then
    var_names+=("${BASH_REMATCH[1]}")
    var_values+=("${BASH_REMATCH[2]}")
  fi
done < "$FILE"

substitute_vars() {
  local text="$1"
  for i in "${!var_names[@]}"; do
    text="${text//\{\{${var_names[$i]}\}\}/${var_values[$i]}}"
  done
  printf '%s' "$text"
}

while IFS= read -r line; do
  trimmed="${line#"${line%%[![:space:]]*}"}"
  trimmed="${trimmed%"${trimmed##*[![:space:]]}"}"

  if [ "$phase" = "pre" ]; then
    [ -z "$trimmed" ] && continue
    [[ "$trimmed" =~ ^# ]] && continue
    [[ "$trimmed" =~ ^// ]] && continue
    [[ "$trimmed" =~ ^@ ]] && continue
    if [[ "$trimmed" =~ ^(GET|POST|PUT|DELETE|PATCH|HEAD|OPTIONS|CONNECT|TRACE)[[:space:]] ]]; then
      method="${trimmed%% *}"
      rest="${trimmed#* }"
      url="${rest%% *}"
      phase="headers"
    fi
    continue
  fi

  if [ "$phase" = "headers" ]; then
    if [ -z "$trimmed" ]; then
      phase="body"
      continue
    fi
    [[ "$trimmed" =~ ^[A-Za-z0-9_-]+: ]] && headers+=("$trimmed")
    continue
  fi

  if [ "$phase" = "body" ]; then
    if [ -n "$body" ]; then
      body="$body
$line"
    else
      body="$line"
    fi
  fi
done <<< "$block"

if [ -z "$method" ] || [ -z "$url" ]; then
  echo "No request found at line $LINE" > "$RESPONSE_FILE"
  exit 1
fi

url=$(substitute_vars "$url")

curl_cmd=(curl -s -i -w '\n---\nTime: %{time_total}s | Size: %{size_download} bytes\n' -X "$method")

for h in "${headers[@]}"; do
  h=$(substitute_vars "$h")
  curl_cmd+=(-H "$h")
done

if [ -n "$body" ]; then
  body_trimmed=$(echo "$body" | sed '/^[[:space:]]*$/d')
  if [ -n "$body_trimmed" ]; then
    body_trimmed=$(substitute_vars "$body_trimmed")
    curl_cmd+=(-d "$body_trimmed")
  fi
fi

curl_cmd+=("$url")

{
  echo "# $method $url"
  echo ""
  "${curl_cmd[@]}" 2>&1
} > "$RESPONSE_FILE"

# Open in Zed only if it's not already open (avoids duplicate tabs)
if command -v zed &>/dev/null; then
  if ! lsof -c Zed 2>/dev/null | grep -q "response.http"; then
    zed "$RESPONSE_FILE"
  fi
fi
