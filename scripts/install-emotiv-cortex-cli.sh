#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "${script_dir}/.." && pwd)"

if ! command -v cargo >/dev/null 2>&1; then
  echo "error: cargo is not installed or not on PATH" >&2
  exit 1
fi

install_root="${EMOTIV_CLI_INSTALL_ROOT:-${CARGO_HOME:-$HOME/.cargo}}"

# Some environments point TMP/TEMP/TMPDIR to non-writable locations.
tmp_root="${TMPDIR:-${TMP:-${TEMP:-/tmp}}}"
if [[ ! -w "${tmp_root}" ]]; then
  tmp_root=/tmp
fi
export TMPDIR="${tmp_root}"
export TMP="${tmp_root}"
export TEMP="${tmp_root}"

echo "Installing emotiv-cortex-cli to: ${install_root}/bin"
cargo install \
  --path "${repo_root}/crates/emotiv-cortex-cli" \
  --root "${install_root}" \
  --force \
  "$@"

bin_dir="${install_root}/bin"
case ":$PATH:" in
  *":${bin_dir}:"*) ;;
  *)
    echo
    echo "Add this to your shell profile to run emotiv-cortex-cli from anywhere:"
    echo "  export PATH=\"${bin_dir}:\$PATH\""
    ;;
esac

echo
echo "Installed. Try:"
echo "  emotiv-cortex-cli --help"
