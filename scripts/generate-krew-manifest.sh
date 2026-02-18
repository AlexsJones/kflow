#!/bin/bash
# Generate krew plugin manifest for kflow
# Usage: ./generate-krew-manifest.sh v0.0.9

set -e

VERSION=${1:-v0.0.9}
REPO="AlexsJones/kflow"
BASE_URL="https://github.com/${REPO}/releases/download/${VERSION}"

# Read SHA256 from artifact files (expects them in artifacts/ dir)
get_sha256() {
  local file="artifacts/$1.tar.gz.sha256"
  if [[ -f "$file" ]]; then
    cut -d' ' -f1 < "$file"
  else
    echo "PLACEHOLDER_SHA256_$1"
  fi
}

SHA_LINUX_AMD64=$(get_sha256 "kflow-linux-amd64")
SHA_LINUX_ARM64=$(get_sha256 "kflow-linux-arm64")
SHA_DARWIN_AMD64=$(get_sha256 "kflow-darwin-amd64")
SHA_DARWIN_ARM64=$(get_sha256 "kflow-darwin-arm64")

cat <<EOF
apiVersion: krew.googlecontainertools.github.com/v1alpha2
kind: Plugin
metadata:
  name: kflow
spec:
  version: "${VERSION}"
  homepage: https://github.com/${REPO}
  shortDescription: Network traffic visualization for Kubernetes
  description: |
    kflow is like 'top' for Kubernetes networking. It reads kernel conntrack
    tables via node-local daemons and visualizes per-node network connections
    in a terminal UI.

    Features:
    - Real-time connection tracking across all nodes
    - TUI with connection details, throughput metrics
    - Easy DaemonSet installation via 'kubectl kflow install'

    Quick start:
      kubectl kflow install -n monitoring
      kubectl kflow

  caveats: |
    This plugin requires a DaemonSet to be installed in your cluster.
    Run 'kubectl kflow install -n <namespace>' to deploy the node agents.
    
    The DaemonSet requires elevated privileges (NET_ADMIN, hostPath /proc)
    to read kernel conntrack tables. Cluster-admin permissions are typically
    needed to install it.

  platforms:
    - bin: kflow
      uri: ${BASE_URL}/kflow-linux-amd64.tar.gz
      sha256: "${SHA_LINUX_AMD64}"
      selector:
        matchLabels:
          os: linux
          arch: amd64
    - bin: kflow
      uri: ${BASE_URL}/kflow-linux-arm64.tar.gz
      sha256: "${SHA_LINUX_ARM64}"
      selector:
        matchLabels:
          os: linux
          arch: arm64
    - bin: kflow
      uri: ${BASE_URL}/kflow-darwin-amd64.tar.gz
      sha256: "${SHA_DARWIN_AMD64}"
      selector:
        matchLabels:
          os: darwin
          arch: amd64
    - bin: kflow
      uri: ${BASE_URL}/kflow-darwin-arm64.tar.gz
      sha256: "${SHA_DARWIN_ARM64}"
      selector:
        matchLabels:
          os: darwin
          arch: arm64
EOF
