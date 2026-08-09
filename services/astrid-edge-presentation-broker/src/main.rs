use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::PathBuf;

use astrid_edge_presentation_broker::{
    Broker, BrokerConfig, BrokerRequest, ClientFormat, ClientOptions, PresentationView, run_client,
};

const MAX_TRUSTED_REPORT_BYTES: usize = 256 * 1024;

fn main() {
    if let Err(error) = run() {
        eprintln!("astrid-edge-presentation-broker: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args_os();
    let _program = arguments.next();
    match arguments
        .next()
        .and_then(|value| value.into_string().ok())
        .as_deref()
    {
        Some("serve") => run_server(&arguments.collect::<Vec<_>>()),
        Some("client") => run_client_command(&arguments.collect::<Vec<_>>()),
        _ => Err("usage: astrid-edge-presentation-broker serve|client ...".into()),
    }
}

fn run_server(arguments: &[OsString]) -> Result<(), Box<dyn std::error::Error>> {
    if arguments.len() != 2 || arguments[0] != "--config" {
        return Err("usage: astrid-edge-presentation-broker serve --config ABSOLUTE_PATH".into());
    }
    let config_path = PathBuf::from(&arguments[1]);
    let config = BrokerConfig::from_root_owned_file(&config_path)?;
    let request_bound = config
        .policy
        .maximum_request_bytes
        .checked_add(1)
        .ok_or("presentation request bound overflow")?;
    let mut request_bytes = Vec::with_capacity(config.policy.maximum_request_bytes);
    std::io::stdin()
        .take(u64::try_from(request_bound)?)
        .read_to_end(&mut request_bytes)?;
    if request_bytes.len() > config.policy.maximum_request_bytes {
        return Err("presentation request exceeds its immutable byte bound".into());
    }
    let request: BrokerRequest = serde_json::from_slice(&request_bytes)?;
    let broker = Broker::new(config)?;
    let response = broker.run(&request);
    response.validate_binding()?;
    eprintln!(
        "candidate_generated_untrusted_presentation status={} generation={} report_projection_sha256={} entrypoint={} binding_sha256={} authority=presentation_only",
        response.status.as_str(),
        response.generation_id.as_deref().unwrap_or("unavailable"),
        response
            .report_projection_sha256
            .as_deref()
            .unwrap_or("initial_generation_inventory_bound"),
        response.entrypoint,
        response.binding_sha256,
    );
    let mut output = serde_json::to_vec(&response)?;
    output.push(b'\n');
    std::io::stdout().write_all(&output)?;
    Ok(())
}

fn run_client_command(arguments: &[OsString]) -> Result<(), Box<dyn std::error::Error>> {
    let mut appliance_id = None;
    let mut view = None;
    let mut window_minutes = None;
    let mut limit = None;
    let mut format = None;
    let mut index = 0_usize;
    while index < arguments.len() {
        let flag = arguments[index]
            .to_str()
            .ok_or("client option is not UTF-8")?;
        let value_index = index.checked_add(1).ok_or("client option index overflow")?;
        let value = arguments
            .get(value_index)
            .and_then(|value| value.to_str())
            .ok_or("client option omitted its UTF-8 value")?;
        match flag {
            "--appliance-id" if appliance_id.is_none() => appliance_id = Some(value.to_owned()),
            "--view" if view.is_none() => view = Some(parse_view(value)?),
            "--window-minutes" if window_minutes.is_none() => {
                window_minutes = Some(value.parse::<u16>()?);
            },
            "--limit" if limit.is_none() => limit = Some(value.parse::<u16>()?),
            "--format" if format.is_none() => format = Some(parse_format(value)?),
            _ => return Err("client options are unknown, repeated, or malformed".into()),
        }
        index = index.checked_add(2).ok_or("client option index overflow")?;
    }
    let options = ClientOptions {
        appliance_id: appliance_id.ok_or("--appliance-id is required")?,
        view: view.ok_or("--view is required")?,
        window_minutes: window_minutes.ok_or("--window-minutes is required")?,
        limit: limit.ok_or("--limit is required")?,
        format: format.ok_or("--format is required")?,
    };
    let input_bound = MAX_TRUSTED_REPORT_BYTES
        .checked_add(1)
        .ok_or("trusted report bound overflow")?;
    let mut trusted_report = Vec::with_capacity(MAX_TRUSTED_REPORT_BYTES);
    std::io::stdin()
        .take(u64::try_from(input_bound)?)
        .read_to_end(&mut trusted_report)?;
    if trusted_report.len() > MAX_TRUSTED_REPORT_BYTES {
        return Err("trusted report input exceeded its immutable bound".into());
    }
    let rendered = run_client(&options, &trusted_report)?;
    std::io::stdout().write_all(rendered.as_bytes())?;
    std::io::stdout().write_all(b"\n")?;
    Ok(())
}

fn parse_view(value: &str) -> Result<PresentationView, Box<dyn std::error::Error>> {
    match value {
        "appliance" => Ok(PresentationView::Appliance),
        "activity" => Ok(PresentationView::Activity),
        "at-a-glance" => Ok(PresentationView::AtAGlance),
        _ => Err("--view must be appliance, activity, or at-a-glance".into()),
    }
}

fn parse_format(value: &str) -> Result<ClientFormat, Box<dyn std::error::Error>> {
    match value {
        "text" => Ok(ClientFormat::Text),
        "key-value" => Ok(ClientFormat::KeyValue),
        "json" => Ok(ClientFormat::Json),
        _ => Err("--format must be text, key-value, or json".into()),
    }
}
