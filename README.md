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
