use anyhow::{Result, anyhow};
use clap::{Parser, builder::ValueParser};
use k8s_openapi::api::{batch::v1::Job, core::v1::Pod};
use kube::{
    Api, Client, Config, Error,
    api::{Patch, PatchParams},
    core::{ObjectMeta, PartialObjectMetaExt},
};
use regex::Regex;
use std::ffi::OsStr;
use std::process::ExitCode;
use std::time::Duration;
use tokio::process::Command;

/// Parse a kubernetes resource label
fn parse_label(v: &str) -> Result<(String, String)> {
    // Reference:
    // https://kubernetes.io/docs/concepts/overview/working-with-objects/labels/#syntax-and-character-set
    // The following regex doesn't quite match the spec: it's more permissive
    if !Regex::new(
        r"^([a-z0-9A-Z.]{1,253}/)?[a-z0-9A-Z\-_.]{0,62}[a-z0-9A-Z]=[a-z0-9A-Z\-_.]{0,62}[a-z0-9A-Z]$",
    )?
    .is_match(v)
    {
        Err(anyhow!("Invalid label value"))
    } else {
        let mut parts = v.split("=");
        Ok((
            String::from(parts.next().unwrap_or_default()),
            String::from(parts.next().unwrap_or_default()),
        ))
    }
}

#[derive(Clone, Debug)]
enum ResourceKind {
    Pod,
    Job,
}

/// Parse a kubernetes resource identifier, limited to jobs and pods only
fn parse_resource(v: &str) -> Result<(ResourceKind, String)> {
    let mut parts = v.split("/");
    let kind = match parts.next() {
        Some("job") => Ok(ResourceKind::Job),
        Some("pod") => Ok(ResourceKind::Pod),
        _ => Err(anyhow!("invalid or missing resource kind")),
    }?;
    Ok((kind, parts.collect::<Vec<_>>().join("/")))
}

/// Wrap a command in a post-success handler that updates a K8s
/// resource label.
#[derive(Parser)]
#[command(version, about)]
pub struct Cli {
    #[arg(short, long, env = "K8S_PSL_NAMESPACE", default_value_t = String::from("default"))]
    namespace: String,

    #[arg(short, long, env = "K8S_PSL_LABEL", value_parser = ValueParser::new(parse_label))]
    label: (String, String),

    #[arg(value_parser = ValueParser::new(parse_resource))]
    resource: (ResourceKind, String),

    #[arg(trailing_var_arg = true)]
    command: Vec<String>,
}

/// Patch a resource of the given kind, so that the given label is
/// added to its metadata. Return an ExitCode somewhat representing
/// the result of the patching.
macro_rules! patch_resource {
    ($kind:ty, $client:expr, $ns:expr, $name:expr, $label:expr) => {{
        let api: Api<$kind> = Api::namespaced($client, $ns);
        let metadata = ObjectMeta {
            labels: Some([$label].into()),
            ..Default::default()
        }
        .into_request_partial::<$kind>();
        let params = PatchParams::apply("k8s-psl");
        let result = api
            .patch_metadata($name, &params, &Patch::Apply(&metadata))
            .await;
        match result {
            Err(Error::Api(_)) => Ok(ExitCode::from(66)),
            Err(Error::Service(_)) => Ok(ExitCode::from(68)),
            Err(_) => Ok(ExitCode::from(1)),
            _ => Ok(ExitCode::from(0)),
        }
    }};
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<ExitCode> {
    let cli = Cli::parse();
    let mut k8s_config = Config::infer().await?;
    k8s_config.connect_timeout = Some(Duration::from_secs(15));
    k8s_config.read_timeout = Some(Duration::from_secs(15));
    k8s_config.write_timeout = Some(Duration::from_secs(15));
    let k8s_client = Client::try_from(k8s_config)?;

    let mut command_parts = cli.command.iter();
    let status = Command::new(OsStr::new(
        command_parts.next().ok_or(anyhow!("Missing command"))?,
    ))
    .args(command_parts)
    .status()
    .await?;
    if !status.success() {
        return Ok(ExitCode::from(u8::try_from(status.code().unwrap_or(1))?));
    }

    match cli.resource.0 {
        ResourceKind::Pod => {
            patch_resource!(Pod, k8s_client, &cli.namespace, &cli.resource.1, cli.label)
        }
        ResourceKind::Job => {
            patch_resource!(Job, k8s_client, &cli.namespace, &cli.resource.1, cli.label)
        }
    }
}
