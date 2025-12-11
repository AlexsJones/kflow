kflow — node-local network "top"

kflow is like top for Kubernetes networking. 
It finds connections through conntrack on your nodes and identifies point to point connections across those nodes. It is a tool for debugging and diagnostics.

__Coming soon: Throughput metrics to rank connections__

![demo](output.gif)

## Installation

```
cargo install kflow
kflow install
kflow # opens tui
```


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

Quick debugging checklist if pods show no connections:

1. On the node, check that conntrack is present and readable: `sudo head -n 20 /proc/net/nf_conntrack` (or your distro's path).
2. Check the pod sees the same file: `kubectl exec -n <ns> <pod> -- ls -l /host/proc/net` and `kubectl exec -n <ns> <pod> -- head -n 5 /host/proc/net/nf_conntrack`.
3. If the file is at a different path on the host, use `kflow install --conntrack <path>` where `<path>` is the host's path (our installer translates `/proc/...` to the mounted `/host/proc/...` for you).
4. Some lightweight clusters (kind, k3s default configurations) may not enable conntrack by default; enable the kernel module or use a cluster that supports conntrack for full visibility.
