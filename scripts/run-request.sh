#!/usr/bin/env bash
# Usage: run-request.sh <file> <line>
# Extracts the HTTP request block containing the given line and executes it with curl.

FILE="$1"
LINE="$2"

if [ -z "$FILE" ] || [ -z "$LINE" ]; then
  echo "Usage: run-request.sh <file> <line>"
  exit 1
fi

# Find the request block boundaries (between ### separators)
block_start=1
block_end=$(wc -l < "$FILE")

# Find the ### before our line
while IFS= read -r num; do
  if [ "$num" -lt "$LINE" ]; then
    block_start=$((num + 1))
  fi
done < <(grep -n '^###' "$FILE" | cut -d: -f1)

# Find the ### after our line
while IFS= read -r num; do
  if [ "$num" -ge "$LINE" ]; then
    block_end=$((num - 1))
    break
  fi
done < <(grep -n '^###' "$FILE" | cut -d: -f1)

# Extract the block
block=$(sed -n "${block_start},${block_end}p" "$FILE")

# Parse: skip comments, blank lines, @variables; find request line
method=""
url=""
declare -a headers=()
body=""
phase="pre" # pre -> request_line -> headers -> body

# Collect file variables from entire file for substitution
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
  echo "$text"
}

while IFS= read -r line; do
  trimmed=$(echo "$line" | sed 's/^[[:space:]]*//' | sed 's/[[:space:]]*$//')

  if [ "$phase" = "pre" ]; then
    # Skip blank lines, comments, annotations, variables
    [ -z "$trimmed" ] && continue
    [[ "$trimmed" =~ ^# ]] && continue
    [[ "$trimmed" =~ ^// ]] && continue
    [[ "$trimmed" =~ ^@ ]] && continue

    # This should be the request line
    if [[ "$trimmed" =~ ^(GET|POST|PUT|DELETE|PATCH|HEAD|OPTIONS|CONNECT|TRACE)[[:space:]] ]]; then
      method=$(echo "$trimmed" | awk '{print $1}')
      url=$(echo "$trimmed" | awk '{print $2}')
      phase="headers"
    fi
    continue
  fi

  if [ "$phase" = "headers" ]; then
    if [ -z "$trimmed" ]; then
      phase="body"
      continue
    fi
    if [[ "$trimmed" =~ ^[A-Za-z0-9_-]+: ]]; then
      headers+=("$trimmed")
    fi
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
  echo "No request found at line $LINE"
  exit 1
fi

# Substitute variables
url=$(substitute_vars "$url")
body=$(substitute_vars "$body")

# Build curl command
curl_args=(-s -i -w '\n---\nTime: %{time_total}s | Size: %{size_download} bytes\n' -X "$method")

for h in "${headers[@]}"; do
  h=$(substitute_vars "$h")
  curl_args+=(-H "$h")
done

if [ -n "$body" ]; then
  body_trimmed=$(echo "$body" | sed '/^[[:space:]]*$/d')
  if [ -n "$body_trimmed" ]; then
    curl_args+=(-d "$body_trimmed")
  fi
fi

echo ">>> $method $url"
echo ""
curl "${curl_args[@]}" "$url"
