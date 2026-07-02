#!/usr/bin/env bash
# =============================================================================
# QuantaChain Testnet — Validator Setup Script
# =============================================================================
# Usage:
#   bash setup-validator.sh             # interactive setup
#   bash setup-validator.sh --dry-run   # preview commands without executing
#
# What this script does:
#   1. Checks Docker is installed and running
#   2. Pulls the latest quanta-node image
#   3. Generates a raw Falcon-512 validator wallet (validator.qua)
#   4. Exports your public key + address to validator.json
#   5. Prints your validator.json so you can send it to the coordinator
#
# Security: Review this script before running it.
#   sha256sum setup-validator.sh  →  verify against the hash on Discord/docs
# =============================================================================

set -euo pipefail

# ── Colours ──────────────────────────────────────────────────────────────────
RED='\033[0;31m'
GRN='\033[0;32m'
YLW='\033[1;33m'
BLU='\033[0;34m'
CYN='\033[0;36m'
BOLD='\033[1m'
RST='\033[0m'

IMAGE="xd637/quanta-node:latest"
WALLET_FILE="validator.qua"
OUT_FILE="validator.json"
DRY_RUN=false

# ── Parse args ────────────────────────────────────────────────────────────────
for arg in "$@"; do
  case $arg in
    --dry-run) DRY_RUN=true ;;
    --help|-h)
      echo "Usage: bash setup-validator.sh [--dry-run]"
      echo "  --dry-run   Print all commands without executing them"
      exit 0
      ;;
    *)
      echo -e "${RED}Unknown argument: $arg${RST}"
      exit 1
      ;;
  esac
done

# ── Helpers ───────────────────────────────────────────────────────────────────
run() {
  if [ "$DRY_RUN" = true ]; then
    echo -e "${YLW}[dry-run]${RST} $*"
  else
    "$@"
  fi
}

banner() {
  echo ""
  echo -e "${CYN}${BOLD}══════════════════════════════════════════════════${RST}"
  echo -e "${CYN}${BOLD}  QuantaChain Testnet — Validator Setup${RST}"
  echo -e "${CYN}${BOLD}  Post-Quantum Falcon-512 Key Generation${RST}"
  echo -e "${CYN}${BOLD}══════════════════════════════════════════════════${RST}"
  echo ""
}

step() {
  echo ""
  echo -e "${BLU}${BOLD}▶ $1${RST}"
}

ok() {
  echo -e "${GRN}  ✔ $1${RST}"
}

warn() {
  echo -e "${YLW}  ⚠ $1${RST}"
}

die() {
  echo -e "${RED}${BOLD}  ✘ ERROR: $1${RST}"
  exit 1
}

# ── Dry-run notice ────────────────────────────────────────────────────────────
banner

if [ "$DRY_RUN" = true ]; then
  echo -e "${YLW}${BOLD}  DRY-RUN MODE — no commands will be executed${RST}"
  echo ""
fi

# ── Step 1: Check Docker ───────────────────────────────────────────────────────
step "Step 1/4 — Checking Docker"

if ! command -v docker &>/dev/null; then
  die "Docker is not installed. Install it with:\n\n    curl -fsSL https://get.docker.com | sh && sudo usermod -aG docker \$USER\n\n  Then log out and back in, and re-run this script."
fi
ok "Docker found: $(docker --version)"

if [ "$DRY_RUN" = false ]; then
  if ! docker info &>/dev/null; then
    die "Docker daemon is not running. Start it with: sudo systemctl start docker"
  fi
  ok "Docker daemon is running"
fi

# ── Step 2: Pull image ────────────────────────────────────────────────────────
step "Step 2/4 — Pulling quanta-node image"
echo "    Image: ${IMAGE}"

run docker pull "${IMAGE}"
ok "Image ready"

# ── Step 3: Generate validator wallet ─────────────────────────────────────────
step "Step 3/4 — Generating your Falcon-512 validator wallet"

WORK_DIR="$(pwd)"

if [ "$DRY_RUN" = false ] && [ -f "${WORK_DIR}/${WALLET_FILE}" ]; then
  warn "wallet file '${WALLET_FILE}' already exists in $(pwd)"
  echo -ne "  Overwrite it? This will destroy the existing key! [y/N]: "
  read -r confirm
  if [[ ! "$confirm" =~ ^[Yy]$ ]]; then
    echo "  Skipping wallet generation. Using existing ${WALLET_FILE}."
    SKIP_WALLET=true
  else
    SKIP_WALLET=false
  fi
else
  SKIP_WALLET=false
fi

if [ "$SKIP_WALLET" = false ]; then
  echo ""
  echo -e "  ${YLW}You will be prompted to set a wallet password.${RST}"
  echo -e "  ${YLW}Save it securely — it cannot be recovered.${RST}"
  echo ""

  run docker run --rm -it \
    -v "${WORK_DIR}:/home/quanta/keys" \
    "${IMAGE}" \
    quanta-wallet new-raw --file /home/quanta/keys/"${WALLET_FILE}"
fi

if [ "$DRY_RUN" = false ] && [ ! -f "${WORK_DIR}/${WALLET_FILE}" ]; then
  die "Wallet file was not created. Check the output above for errors."
fi
ok "Wallet file: ${WORK_DIR}/${WALLET_FILE}"

# ── Step 4: Export validator.json ─────────────────────────────────────────────
step "Step 4/4 — Exporting your validator public key"

run docker run --rm -it \
  -v "${WORK_DIR}:/home/quanta/keys" \
  "${IMAGE}" \
  quanta-wallet export-validator \
    --wallet /home/quanta/keys/"${WALLET_FILE}" \
    --out    /home/quanta/keys/"${OUT_FILE}"

if [ "$DRY_RUN" = false ] && [ ! -f "${WORK_DIR}/${OUT_FILE}" ]; then
  die "validator.json was not created. Check the output above for errors."
fi
ok "Exported: ${WORK_DIR}/${OUT_FILE}"

# ── Done ──────────────────────────────────────────────────────────────────────
echo ""
echo -e "${CYN}${BOLD}══════════════════════════════════════════════════${RST}"
echo -e "${GRN}${BOLD}  ✔ Setup complete!${RST}"
echo -e "${CYN}${BOLD}══════════════════════════════════════════════════${RST}"
echo ""

if [ "$DRY_RUN" = false ]; then
  echo -e "${BOLD}  Your validator.json (send this to the coordinator):${RST}"
  echo ""
  cat "${WORK_DIR}/${OUT_FILE}"
  echo ""
fi

echo -e "  ${YLW}${BOLD}⚠ IMPORTANT:${RST}"
echo -e "  ${YLW}  • Keep '${WALLET_FILE}' and your password safe — this IS your private key.${RST}"
echo -e "  ${YLW}  • Send ONLY '${OUT_FILE}' to the coordinator, never '${WALLET_FILE}'.${RST}"
echo ""
echo -e "  ${BLU}Next steps:${RST}"
echo -e "  ${BLU}  • DM your validator.json to the coordinator (K) on Telegram/Discord${RST}"
echo -e "  ${BLU}  • Wait for the genesis.json + node launch instructions${RST}"
echo -e "  ${BLU}  • Discord: https://discord.gg/7KmMBrrJEz${RST}"
echo ""
