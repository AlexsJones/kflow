kflow — node-local network "top"

kflow is like top for Kubernetes networking. 
It finds connections through conntrack on your nodes and identifies point to point connections across those nodes. It is a tool for debugging and diagnostics.

__Coming soon: Throughput metrics to rank connections__

![demo](output.gif)

## Installation

### Via Krew (kubectl plugin)

```bash
kubectl krew install kflow
kubectl kflow install -n monitoring  # Deploy DaemonSet
kubectl kflow                        # Open TUI
```

### Via Homebrew

```bash
brew install AlexsJones/kflow/kflow
kflow install -n monitoring
kflow
```

### Via Cargo

```bash
cargo install kflow
kflow install  # Installs the daemonset
kflow          # opens tui
```

_Press h for keybinding information_


### Privileges

The agent intentionally requires elevated privileges on the node. The DaemonSet mounts the host `/proc` into each pod, runs the container as root, and requests NET_ADMIN/NET_RAW capabilities so it can read live conntrack state. Applying the provided Kubernetes manifest therefore requires a user with permission to create DaemonSets and hostPath mounts in the target namespace (cluster-admin or equivalent RBAC is usually needed).


### Build yourself

Build the CLI and daemon locally with Cargo. The repository contains a multi-stage `Dockerfile.daemon` and a `k8s/daemonset.yaml` manifest; the CLI provides `install` and `uninstall` subcommands that call `kubectl` for convenience.

To build and run the CLI (the binary is named `kflow`):

```sh
cargo build --bin kflow
./target/debug/kflow
```

## Installing the Daemonset

Install the DaemonSet into the current cluster context (may require cluster-admin). The installer accepts an optional `--conntrack` value to override the path the daemon reads from inside the pod:

```sh
kflow install -n <namespace>
kflow install --conntrack /proc/net/ip_conntrack -n <namespace>
```

Remove the DaemonSet:

```sh
kflow uninstall -n <namespace>
```

Notes: some environments (for example kind) may not expose conntrack entries by default or may use a different proc path. If pods show no connections, verify conntrack is present on the node (`sudo head -n 20 /proc/net/nf_conntrack`) and that the manifest is mounting `/proc` into `/host/proc` inside the pod.

## Conntrack requirement and path locations

kflow relies on the kernel conntrack table being available on each node so the node-local daemon can read active connections. Many Linux distributions expose conntrack under `/proc/net/` but the exact filename and location can vary by kernel/module and distribution.

Common paths you may encounter:

- `/proc/net/nf_conntrack` (modern kernels, common on many distros)
- `/proc/net/ip_conntrack` (older kernels or different module naming)
- `/proc/net/nf_conntrack6` (IPv6 conntrack on some systems)

If you run the provided DaemonSet the manifest mounts the host `/proc` into the pod at `/host/proc` and sets the default `CONNTRACK_PATH` to `/host/proc/net/nf_conntrack`. If you have a different host path, supply a container-visible path to the installer using `--conntrack`.

Examples:

- Node exposes the file at `/proc/net/nf_conntrack` (default):

	`kflow install -n monitoring`

- Node exposes the file at `/proc/net/ip_conntrack` (override):

	`kflow install --conntrack /proc/net/ip_conntrack -n monitoring`

- You mounted host `/proc` at a different location inside the pod (advanced):

	Edit `k8s/daemonset.yaml` so the volumeMount and `CONNTRACK_PATH` agree, or pass the exact path the daemon can see inside the container with `--conntrack`.

### Enable conntrack accounting (bytes)

To see non-zero per-connection byte counters and throughput in kflow, you must enable conntrack accounting on each node. This is a kernel setting and must be configured on the host (we do not change it from inside the pod).

**For kind clusters:**

```sh
# Enable on control-plane
docker exec kind-control-plane sh -c 'echo 1 > /proc/sys/net/netfilter/nf_conntrack_acct'

# Enable on each worker node
docker exec kind-worker sh -c 'echo 1 > /proc/sys/net/netfilter/nf_conntrack_acct'
docker exec kind-worker2 sh -c 'echo 1 > /proc/sys/net/netfilter/nf_conntrack_acct'
# repeat for all workers
```

**For regular nodes:**

```sh
sudo sh -c 'echo 1 > /proc/sys/net/netfilter/nf_conntrack_acct'
```

Make it persistent across reboots via sysctl:

```sh
echo 'net.netfilter.nf_conntrack_acct=1' | sudo tee /etc/sysctl.d/99-kflow.conf
sudo sysctl --system
```

**Verify bytes are recorded:**

On the node:
```sh
sudo head -5 /proc/net/nf_conntrack
```

Or from a kflow pod:
```sh
kubectl exec -n <namespace> <kflow-pod> -- head -5 /host/proc/net/nf_conntrack
```

If `bytes=` fields are present and increasing (e.g., `bytes=1234 packets=10`), kflow will compute per-connection throughput automatically. **Note:** Only NEW connections created after enabling accounting will show byte counters.

**Troubleshooting bytes=0:**

If you see `bytes: 0` and `throughput_bytes_per_sec: 0` in kflow even after enabling `nf_conntrack_acct`:

1. Verify accounting is enabled: `cat /proc/sys/net/netfilter/nf_conntrack_acct` should return `1`
2. Check if bytes appear in conntrack output: `sudo cat /proc/net/nf_conntrack | grep bytes`
3. The setting only affects *new* connections. Existing connections won't show bytes retroactively. Generate new traffic or wait for connections to be re-established.
4. Check daemon logs for sample conntrack lines: `kubectl logs -n <namespace> <pod-name>` (with `KFLOW_DEBUG=true`)

### Auto-detect mode

The daemon can attempt to auto-detect the correct conntrack file path if you don't want to pick an exact path. 

**This is enabled by default.**


Use the installer to embed a custom path into the manifest (the installer will translate `/proc/...` to `/host/proc/...` when needed):

	`kflow install --conntrack <whatever> -n monitoring`

If auto-detection fails the daemon will log a message and fall back to the configured path; using `KFLOW_DEBUG` will emit helpful debug messages about which candidate paths were tested.

Quick debugging checklist if pods show no connections:

1. On the node, check that conntrack is present and readable: `sudo head -n 20 /proc/net/nf_conntrack` (or your distro's path).
2. Check the pod sees the same file: `kubectl exec -n <ns> <pod> -- ls -l /host/proc/net` and `kubectl exec -n <ns> <pod> -- head -n 5 /host/proc/net/nf_conntrack`.
3. If the file is at a different path on the host, use `kflow install --conntrack <path>` where `<path>` is the host's path (our installer translates `/proc/...` to the mounted `/host/proc/...` for you).
4. Some lightweight clusters (kind, k3s default configurations) may not enable conntrack by default; enable the kernel module or use a cluster that supports conntrack for full visibility.
