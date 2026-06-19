use aws_config::BehaviorVersion;
use aws_credential_types::provider::ProvideCredentials;
use aws_sigv4::http_request::{sign, SignableBody, SignableRequest, SignatureLocation, SigningSettings};
use aws_sigv4::sign::v4;
use clap::{Parser, ValueEnum};
use regex::Regex;
use std::time::SystemTime;
use tracing::{info, error, debug};

#[derive(Clone, Debug, ValueEnum)]
enum OutputType {
    SmartCrop,
    Subtitles,
    EventClip,
}

#[derive(Parser)]
#[command(name = "inference-metadata", about = "Query metadata from Elemental Inference")]
struct Cli {
    /// Elemental Inference feed ID
    #[arg(short, long)]
    feed_id: String,

    /// Data endpoint
    #[arg(short, long)]
    endpoint: String,

    /// Output type to query
    #[arg(short, long)]
    output_type: OutputType,

    /// Start PTS in milliseconds
    #[arg(long, default_value_t = 0)]
    start: u64,

    /// End PTS in milliseconds
    #[arg(long, default_value_t = 5000)]
    end: u64,

    /// Frame rate numerator (smart crop only)
    #[arg(long, default_value_t = 30)]
    fps: u32,

    /// AWS region
    #[arg(short, long, default_value = "us-east-1")]
    region: String,
}

async fn query_metadata(
    endpoint: &str,
    feed_id: &str,
    region: &str,
    body: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let url = format!("https://{endpoint}/v1/feed/{feed_id}/input/0/metadata");

    debug!(url = %url, "Querying metadata");

    let credentials = config
        .credentials_provider()
        .unwrap()
        .provide_credentials()
        .await?;

    let mut settings = SigningSettings::default();
    settings.signature_location = SignatureLocation::Headers;
    settings.payload_checksum_kind = aws_sigv4::http_request::PayloadChecksumKind::XAmzSha256;

    let identity = credentials.into();
    let signing_params = v4::SigningParams::builder()
        .identity(&identity)
        .region(region)
        .name("elemental-inference")
        .time(SystemTime::now())
        .settings(settings)
        .build()?;

    let body_bytes = body.as_bytes();
    let signable = SignableRequest::new(
        "POST",
        &url,
        [("host", endpoint)].into_iter(),
        SignableBody::Bytes(body_bytes),
    )?;

    let (instructions, _) = sign(signable, &signing_params.into())?.into_parts();

    let mut req = http::Request::builder()
        .method("POST")
        .uri(&url)
        .body(())
        .unwrap();
    instructions.apply_to_request_http1x(&mut req);

    let client = reqwest::Client::new();
    let reqwest_url = reqwest::Url::parse(&url)?;
    let mut final_req = client.post(reqwest_url)
        .header("content-type", "application/json")
        .body(body.to_string());
    for (name, value) in req.headers() {
        final_req = final_req.header(name.as_str(), value.to_str()?);
    }

    let resp = final_req.send().await?;
    let status = resp.status();
    let text = resp.text().await?;

    if !status.is_success() {
        error!(status = %status, response = %text, "Query failed");
    } else {
        debug!(status = %status, "Query success");
    }

    Ok(text)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("inference_metadata=info")
        .init();

    let cli = Cli::parse();

    info!(feed_id = %cli.feed_id, output_type = ?cli.output_type, start = cli.start, end = cli.end, "Querying metadata");

    let body = match cli.output_type {
        OutputType::SmartCrop => format!(
            r#"{{"outputName": "smart-cropping", "timeSpecification": {{"ptsBased": {{"startPts": {}, "endPts": {}, "timescale": 1000}}}}, "parameters": {{"smartCropping": {{"frameRate": {{"numerator": {}, "denominator": 1}}}}}}}}"#,
            cli.start, cli.end, cli.fps
        ),
        OutputType::Subtitles => format!(
            r#"{{"outputName": "subtitling-spa", "timeSpecification": {{"ptsBased": {{"startPts": {}, "endPts": {}, "timescale": 1000}}}}, "parameters": {{"subtitling": {{}}}}}}"#,
            cli.start, cli.end
        ),
        OutputType::EventClip => format!(
            r#"{{"outputName": "event-clipping", "timeSpecification": {{"ptsBased": {{"startPts": {}, "endPts": {}, "timescale": 1000}}}}, "parameters": {{}}}}"#,
            cli.start, cli.end
        ),
    };

    match query_metadata(&cli.endpoint, &cli.feed_id, &cli.region, &body).await {
        Ok(response) => {
            if matches!(cli.output_type, OutputType::Subtitles) {
                print_parsed_subtitles(&response);
            } else {
                let parsed: serde_json::Value = serde_json::from_str(&response).unwrap_or_default();
                println!("{}", serde_json::to_string_pretty(&parsed).unwrap_or(response));
            }
        }
        Err(e) => error!(error = %e, "Failed to query metadata"),
    }
}

fn print_parsed_subtitles(response: &str) {
    let json: serde_json::Value = match serde_json::from_str(response) {
        Ok(v) => v,
        Err(_) => { info!("{response}"); return; }
    };

    let ttml = json["items"][0]["metadata"]["subtitling"]["ttml"]
        .as_str()
        .unwrap_or("");

    if ttml.is_empty() {
        info!("No subtitle data");
        return;
    }

    // Extract <p begin="..." end="...">..text..</p>
    let p_re = Regex::new(r#"<p[^>]*begin="([^"]+)"[^>]*end="([^"]+)"[^>]*>\s*<span[^>]*>(.*?)</span>\s*</p>"#).unwrap();
    let br_re = Regex::new(r"<br\s*/?>").unwrap();
    let tag_re = Regex::new(r"<[^>]+>").unwrap();

    for cap in p_re.captures_iter(ttml) {
        let begin = &cap[1];
        let end = &cap[2];
        let text = br_re.replace_all(&cap[3], " | ");
        let text = tag_re.replace_all(&text, "");
        info!(begin = %begin, end = %end, text = %text, "subtitle");
    }
}
