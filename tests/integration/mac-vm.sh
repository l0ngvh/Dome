#!/usr/bin/env bash
# One-command local run of the udev integration suite on macOS.
#
# Creates (or reuses) an Ubuntu Lima VM under the finch-bundled limactl, installs
# the build and runtime deps, builds dome inside the VM, and runs ci/udev-integration.sh
# against a DRM node with Mesa software GL. KMS selects the implementation:
#   KMS=vkms   (default) a vz VM plus the vkms module, with the CRTC CRC assertion.
#   KMS=virtio a qemu VM with a virtio-gpu device, a second KMS implementation.
# virtio-gpu-gl (virgl) is unreachable on this host, so both paths use software GL.
# The vkms path installs the Ubuntu HWE kernel and reboots into it once, so vkms
# configfs is present for the connector-hotplug and primary re-home check.
# See plans/linux-testing/02-vm-validation.md.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
LIMACTL="${LIMACTL:-/Applications/Finch/lima/bin/limactl}"
export LIMA_HOME="${LIMA_HOME:-$HOME/.lima}"
# limactl needs the bundled qemu-system-* on PATH to start a qemu VM (the virtio path).
export PATH="$(dirname "$LIMACTL"):$PATH"

KMS="${KMS:-vkms}"
case "$KMS" in
  vkms)   DEFAULT_VM=dome-vm;      VMTYPE=vz;   VIDEO_BLOCK="" ;;
  virtio) DEFAULT_VM=dome-qemu-vm; VMTYPE=qemu; VIDEO_BLOCK=$'video:\n  display: "default"' ;;
  *) echo "unknown KMS=$KMS (use vkms or virtio)" >&2; exit 1 ;;
esac
VM="${DOME_VM:-$DEFAULT_VM}"

[ -x "$LIMACTL" ] || { echo "bundled limactl not found at $LIMACTL; install finch or set LIMACTL" >&2; exit 1; }

echo "==> ensuring VM '$VM' exists and runs (KMS=$KMS)"
if ! "$LIMACTL" list --format '{{.Name}}' 2>/dev/null | grep -qx "$VM"; then
  cfg="$(mktemp)-dome-vm.yaml"
  cat > "$cfg" <<EOF
images:
- location: "https://cloud-images.ubuntu.com/releases/24.04/release/ubuntu-24.04-server-cloudimg-arm64.img"
  arch: "aarch64"
- location: "https://cloud-images.ubuntu.com/releases/24.04/release/ubuntu-24.04-server-cloudimg-amd64.img"
  arch: "x86_64"
vmType: "$VMTYPE"
$VIDEO_BLOCK
cpus: 4
memory: "4GiB"
disk: "20GiB"
mounts:
- location: "$REPO_ROOT"
  mountPoint: "/dome"
  writable: false
EOF
  "$LIMACTL" start "$cfg" --name="$VM" --tty=false
elif [ "$("$LIMACTL" list "$VM" --format '{{.Status}}' 2>/dev/null)" != "Running" ]; then
  "$LIMACTL" start "$VM" --tty=false
fi

# The virtio VM uses video.display: default, which opens a QEMU window on macOS.
# Stop it on exit so the window does not linger. vkms uses vz and needs no stop.
if [ "$KMS" = virtio ]; then
  trap 'echo "==> stopping $VM"; "$LIMACTL" stop "$VM" >/dev/null 2>&1 || true' EXIT
fi

echo "==> provisioning deps and rust (idempotent)"
"$LIMACTL" shell "$VM" -- env KMS="$KMS" bash -s <<'PROV'
set -euo pipefail
sudo apt-get update -qq
pkgs="build-essential pkg-config curl git rsync libwayland-dev libxkbcommon-dev libudev-dev libinput-dev libgbm-dev libdrm-dev libseat-dev libegl-dev libgles-dev libdisplay-info-dev libgl1-mesa-dri seatd weston wl-clipboard kbd xwayland x11-apps clang libxcb1-dev libxcb-cursor-dev"
sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -qq $pkgs >/dev/null
if [ ! -x "$HOME/.cargo/bin/cargo" ]; then
  curl -sSf "https://static.rust-lang.org/rustup/dist/$(uname -m)-unknown-linux-gnu/rustup-init" -o /tmp/rustup-init
  chmod +x /tmp/rustup-init
  /tmp/rustup-init -y --default-toolchain stable --profile minimal >/dev/null
fi
PROV

# vkms configfs needs a kernel around 6.11+, newer than the 6.8 the Ubuntu 24.04 image
# ships. Install the HWE kernel and reboot into it once, then load vkms with modules
# matching the running kernel.
if [ "$KMS" = vkms ]; then
  echo "==> ensuring the HWE kernel (vkms configfs, for connector hotplug)"
  reboot_needed=$("$LIMACTL" shell "$VM" -- sudo bash -c '
    export DEBIAN_FRONTEND=noninteractive
    apt-get install -y -qq linux-generic-hwe-24.04 >/dev/null
    newest=$(ls -1 /boot/vmlinuz-* | sed "s#.*/vmlinuz-##" | sort -V | tail -1)
    [ "$(uname -r)" = "$newest" ] && echo no || echo yes' | tail -1)
  if [ "$reboot_needed" = yes ]; then
    echo "==> rebooting $VM into the HWE kernel"
    "$LIMACTL" shell "$VM" -- sudo reboot >/dev/null 2>&1 || true
    sleep 20
    for _ in $(seq 1 30); do "$LIMACTL" shell "$VM" -- true 2>/dev/null && break; sleep 5; done
  fi
  "$LIMACTL" shell "$VM" -- sudo bash -c '
    export DEBIAN_FRONTEND=noninteractive
    apt-get install -y -qq "linux-modules-extra-$(uname -r)" >/dev/null
    modprobe vkms enable_writeback=1'
fi

echo "==> ensuring xwayland-satellite (for the XWayland check)"
"$LIMACTL" shell "$VM" -- bash -s <<'SAT'
set -euo pipefail
export PATH="$HOME/.cargo/bin:$PATH"
if [ ! -x "$HOME/.cargo/bin/xwayland-satellite" ]; then
  cargo install --git https://github.com/Supreeeme/xwayland-satellite --tag v0.8.2 --locked
fi
sudo ln -sf "$HOME/.cargo/bin/xwayland-satellite" /usr/local/bin/xwayland-satellite
SAT

echo "==> building dome in the VM"
"$LIMACTL" shell "$VM" -- bash -s <<'BUILD'
set -euo pipefail
export PATH="$HOME/.cargo/bin:$PATH"
rsync -a --delete --exclude=target --exclude=.git --exclude=test-out /dome/ "$HOME/dome-src/"
cd "$HOME/dome-src"
cargo build --bin dome
BUILD

phases=all
for drv in llvmpipe softpipe; do
  echo "==> running the udev integration suite (KMS=$KMS, GALLIUM_DRIVER=$drv, TEST_PHASES=$phases)"
  "$LIMACTL" shell "$VM" -- sudo bash -c \
    "DOME_SRC=\"\$(ls -d /home/*/dome-src | head -1)\"; TEST_PHASES=$phases KMS=$KMS GALLIUM_DRIVER=$drv bash /dome/ci/udev-integration.sh \"\$DOME_SRC/target/debug/dome\""
  phases=driver
done
