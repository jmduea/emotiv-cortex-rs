#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "${script_dir}/.." && pwd)"

usage() {
  cat <<EOF
Usage: $(basename "$0") [--lsl] [-- extra-cargo-args...]

Options:
  --lsl     Enable Lab Streaming Layer support (Windows/macOS only)
  --help    Show this help message

Any arguments after -- are forwarded to cargo install.

Examples:
  $(basename "$0")              # install without LSL
  $(basename "$0") --lsl        # install with LSL support
  $(basename "$0") -- --locked  # forward --locked to cargo install
EOF
}

enable_lsl=false
cargo_extra_args=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --lsl)
      enable_lsl=true
      shift
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    --)
      shift
      cargo_extra_args+=("$@")
      break
      ;;
    *)
      cargo_extra_args+=("$1")
      shift
      ;;
  esac
done

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

feature_args=()
if [[ "${enable_lsl}" == "true" ]]; then
  os="$(uname -s)"
  if [[ "${os}" == "Linux" ]]; then
    echo "error: LSL is currently unsupported on Linux. Install without --lsl, or use Windows/macOS." >&2
    exit 1
  fi
  feature_args+=(--features lsl)
  echo "Installing emotiv-cortex-cli (with LSL) to: ${install_root}/bin"
else
  echo "Installing emotiv-cortex-cli to: ${install_root}/bin"
  echo "  Tip: use --lsl to enable Lab Streaming Layer support (Windows/macOS)"
fi

cargo install \
  --path "${repo_root}/crates/emotiv-cortex-cli" \
  --root "${install_root}" \
  --force \
  "${feature_args[@]+"${feature_args[@]}"}" \
  "${cargo_extra_args[@]+"${cargo_extra_args[@]}"}"

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
