#!/bin/bash
# Usage: run-request.sh <file> <line>
# Extracts the HTTP request at the given line and executes it with curl.

FILE="$1"
LINE="$2"

if [ -z "$FILE" ] || [ -z "$LINE" ]; then
  echo "Usage: run-request.sh <file> <line>"
  exit 1
fi

# Read the file and find the request block containing the given line
awk -v target="$LINE" '
BEGIN { in_block=0; method=""; url=""; body=""; reading_headers=1; reading_body=0; block_start=0; found=0 }
/^###/ {
  if (found) exit
  method=""; url=""; body=""; headers=""; reading_headers=1; reading_body=0
  block_start=NR
  next
}
/^(GET|POST|PUT|DELETE|PATCH|HEAD|OPTIONS|CONNECT|TRACE) / {
  if (NR <= target || (block_start > 0 && block_start < target)) {
    method=$1; url=$2; reading_headers=1; reading_body=0; body=""; headers=""
    found=(NR <= target)
    next
  }
}
found && reading_headers && /^[A-Za-z0-9_-]+:/ {
  headers=headers " -H " "\047" $0 "\047"
  next
}
found && reading_headers && /^$/ {
  reading_headers=0; reading_body=1
  next
}
found && reading_body && /^###/ { exit }
found && reading_body {
  if (body != "") body=body "\n"
  body=body $0
}
END {
  if (method == "") { print "No request found at line " target; exit 1 }
  cmd = "curl -s -w \"\n---\nHTTP Status: %{http_code}\nTime: %{time_total}s\nSize: %{size_download} bytes\n\" -X " method
  cmd = cmd headers
  if (body != "") cmd = cmd " -d \047" body "\047"
  cmd = cmd " \047" url "\047"
  print ">>> " method " " url
  print ""
  system(cmd)
}
' "$FILE"
