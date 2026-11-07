#!/usr/bin/env bash
set -euo pipefail

destination=${1:?usage: fetch-geoip.sh OUTPUT}
: "${IPINFO_DOWNLOAD_TOKEN:?Set IPINFO_DOWNLOAD_TOKEN to an IPinfo access token with Lite database downloads enabled}"
database_url=https://ipinfo.io/data/ipinfo_lite.mmdb
partial="$destination.part"

fetch() {
  # Read the token from stdin so it stays out of curl's command-line arguments.
  printf '%s' "$IPINFO_DOWNLOAD_TOKEN" |
    curl --disable --fail --silent --show-error --location --retry 3 \
      --proto '=https' --proto-redir '=https' --connect-timeout 10 --max-time 120 \
      --get --data-urlencode 'token@-' "$@"
}

mkdir -p "$(dirname "$destination")"
# The provider can publish a new snapshot between these two requests.
for attempt in 1 2 3; do
  checksum=$(fetch "$database_url/checksums" |
    jq -er '.checksums.sha256 | strings | select(test("^[0-9A-Fa-f]{64}$")) | ascii_downcase')
  fetch --output "$partial" "$database_url"
  actual=$(shasum -a 256 "$partial")
  if [[ ${actual%% *} == "$checksum" ]]; then
    mv "$partial" "$destination"
    printf 'IPinfo Lite SHA-256: %s\n' "$checksum"
    exit 0
  fi
  printf 'IPinfo Lite checksum mismatch (attempt %s of 3).\n' "$attempt" >&2
done
exit 1
