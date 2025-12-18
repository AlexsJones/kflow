use std::process::Stdio;
use tokio::process::Command;

use crate::cli::DEFAULT_DAEMONSET;

pub async fn discover_pods() -> anyhow::Result<Vec<String>> {
    let output = Command::new("kubectl")
        .args(["get", "pods", "-l", "app=kflow-daemon", "-o", "jsonpath={.items[*].metadata.name}"])
        .output()
        .await?;

    if !output.status.success() {
        anyhow::bail!("kubectl failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    let out = String::from_utf8_lossy(&output.stdout);
    let names: Vec<String> = out.split_whitespace().map(|s| s.to_string()).collect();
    Ok(names)
}

pub async fn run_kubectl_apply(file: Option<&str>, namespace: Option<&str>, conntrack: Option<&str>) -> anyhow::Result<()> {
    if let Some(path) = file {
        if let Some(ct) = conntrack {
            let ct_repl = if ct == "auto" {
                "auto".to_string()
            } else if ct.starts_with("/proc/") {
                format!("/host{}", ct)
            } else {
                ct.to_string()
            };
            let content = std::fs::read_to_string(path)?;
            let replaced = update_manifest_conntrack(&content, &ct_repl);

            let mut args: Vec<String> = vec!["apply".into(), "-f".into(), "-".into()];
            if let Some(ns) = namespace {
                args.push("-n".into());
                args.push(ns.to_string());
            }

            let mut child = Command::new("kubectl").args(args).stdin(Stdio::piped()).spawn()?;
            if let Some(mut stdin) = child.stdin.take() {
                use tokio::io::AsyncWriteExt;
                stdin.write_all(replaced.as_bytes()).await?;
            }
            let output = child.wait_with_output().await?;
            if output.status.success() {
                println!("{}", String::from_utf8_lossy(&output.stdout));
                Ok(())
            } else {
                anyhow::bail!("kubectl apply failed: {}", String::from_utf8_lossy(&output.stderr));
            }
        } else {
            let mut args: Vec<String> = vec!["apply".into(), "-f".into(), path.to_string()];
            if let Some(ns) = namespace {
                args.push("-n".into());
                args.push(ns.to_string());
            }

            let output = Command::new("kubectl").args(args).output().await?;
            if output.status.success() {
                println!("{}", String::from_utf8_lossy(&output.stdout));
                Ok(())
            } else {
                anyhow::bail!("kubectl apply failed: {}", String::from_utf8_lossy(&output.stderr));
            }
        }
    } else {
        let mut manifest = DEFAULT_DAEMONSET.to_string();
        if let Some(ct) = conntrack {
            let ct_repl = if ct == "auto" {
                "auto".to_string()
            } else if ct.starts_with("/proc/") {
                format!("/host{}", ct)
            } else {
                ct.to_string()
            };
            manifest = update_manifest_conntrack(&manifest, &ct_repl);
        }

        let mut args: Vec<String> = vec!["apply".into(), "-f".into(), "-".into()];
        if let Some(ns) = namespace {
            args.push("-n".into());
            args.push(ns.to_string());
        }

        let mut child = Command::new("kubectl").args(args).stdin(Stdio::piped()).spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            use tokio::io::AsyncWriteExt;
            stdin.write_all(manifest.as_bytes()).await?;
        }
        let output = child.wait_with_output().await?;
        if output.status.success() {
            println!("{}", String::from_utf8_lossy(&output.stdout));
            Ok(())
        } else {
            anyhow::bail!("kubectl apply failed: {}", String::from_utf8_lossy(&output.stderr));
        }
    }
}

pub async fn run_kubectl_delete(file: Option<&str>, namespace: Option<&str>, conntrack: Option<&str>) -> anyhow::Result<()> {
    if let Some(path) = file {
            if let Some(ct) = conntrack {
                let ct_repl = if ct == "auto" {
                    "auto".to_string()
                } else if ct.starts_with("/proc/") {
                    format!("/host{}", ct)
                } else {
                    ct.to_string()
                };
                    let content = std::fs::read_to_string(path)?;
                    let replaced = update_manifest_conntrack(&content, &ct_repl);

            let mut args: Vec<String> = vec!["delete".into(), "-f".into(), "-".into()];
            if let Some(ns) = namespace {
                args.push("-n".into());
                args.push(ns.to_string());
            }

            let mut child = Command::new("kubectl").args(args).stdin(Stdio::piped()).spawn()?;
            if let Some(mut stdin) = child.stdin.take() {
                use tokio::io::AsyncWriteExt;
                stdin.write_all(replaced.as_bytes()).await?;
            }
            let output = child.wait_with_output().await?;
            if output.status.success() {
                println!("{}", String::from_utf8_lossy(&output.stdout));
                Ok(())
            } else {
                anyhow::bail!("kubectl delete failed: {}", String::from_utf8_lossy(&output.stderr));
            }
        } else {
            let mut args: Vec<String> = vec!["delete".into(), "-f".into(), path.to_string()];
            if let Some(ns) = namespace {
                args.push("-n".into());
                args.push(ns.to_string());
            }

            let output = Command::new("kubectl").args(args).output().await?;
            if output.status.success() {
                println!("{}", String::from_utf8_lossy(&output.stdout));
                Ok(())
            } else {
                anyhow::bail!("kubectl delete failed: {}", String::from_utf8_lossy(&output.stderr));
            }
        }
    } else {
        let mut manifest = DEFAULT_DAEMONSET.to_string();
        if let Some(ct) = conntrack {
            let ct_repl = if ct == "auto" {
                "auto".to_string()
            } else if ct.starts_with("/proc/") {
                format!("/host{}", ct)
            } else {
                ct.to_string()
            };
            manifest = update_manifest_conntrack(&manifest, &ct_repl);
        }

        let mut args: Vec<String> = vec!["delete".into(), "-f".into(), "-".into()];
        if let Some(ns) = namespace {
            args.push("-n".into());
            args.push(ns.to_string());
        }

        let mut child = Command::new("kubectl").args(args).stdin(Stdio::piped()).spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            use tokio::io::AsyncWriteExt;
            stdin.write_all(manifest.as_bytes()).await?;
        }
        let output = child.wait_with_output().await?;
        if output.status.success() {
            println!("{}", String::from_utf8_lossy(&output.stdout));
            Ok(())
        } else {
            anyhow::bail!("kubectl delete failed: {}", String::from_utf8_lossy(&output.stderr));
        }
    }
}

fn update_manifest_conntrack(manifest: &str, ct_repl: &str) -> String {
    let mut lines: Vec<String> = manifest.lines().map(|s| s.to_string()).collect();
    for i in 0..lines.len() {
        let trimmed = lines[i].trim_start();
        if trimmed.starts_with("name:") && trimmed.contains("CONNTRACK_PATH") {
            for j in (i+1)..((i+6).min(lines.len())) {
                let t = lines[j].trim_start();
                if t.starts_with("value:") {
                    let indent = &lines[j][..lines[j].find(t).unwrap_or(0)];
                    let rest = t["value:".len()..].trim_start();
                    let quote = if rest.starts_with('"') { '"' } else if rest.starts_with('\'') { '\'' } else { '"' };
                    let new_val = if ct_repl == "auto" {
                        format!("value: {}auto{}", quote, quote)
                    } else {
                        format!("value: {}{}{}", quote, ct_repl, quote)
                    };
                    lines[j] = format!("{}{}", indent, new_val);
                    return lines.join("\n");
                }
            }
        }
    }
    manifest.replace("/host/proc/net/nf_conntrack", ct_repl).replace("/proc/net/nf_conntrack", ct_repl)
}
