use aws_config::BehaviorVersion;
use aws_credential_types::provider::ProvideCredentials;
use aws_sdk_elementalinference as ei;
use aws_sigv4::http_request::{
    sign, SignableBody, SignableRequest, SignatureLocation, SigningSettings,
};
use aws_sigv4::sign::v4;
use clap::Parser;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tokio::task::JoinSet;
use tracing::{debug, error, info};

#[derive(Parser)]
#[command(
    name = "inference-ingest",
    about = "Segment and deliver CMAF to Elemental Inference"
)]
struct Cli {
    /// Data endpoint (without trailing slash)
    #[arg(short, long)]
    endpoint: String,

    /// Path to CMAF assets directory
    #[arg(short, long, default_value = "cmaf-output")]
    assets: PathBuf,

    /// AWS region
    #[arg(short, long, default_value = "us-east-1")]
    region: String,
}

/// Create a fresh feed and associate it, returns feed_id
async fn create_and_associate_feed(client: &ei::Client) -> String {
    let resp = client
        .create_feed()
        .name(format!(
            "ingest-{}",
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        ))
        .outputs(
            ei::types::CreateOutput::builder()
                .name("smart-cropping")
                .output_config(ei::types::OutputConfig::Cropping(
                    ei::types::CroppingConfig::builder().build(),
                ))
                .status(ei::types::OutputStatus::Enabled)
                .build()
                .unwrap(),
        )
        .outputs(
            ei::types::CreateOutput::builder()
                .name("subtitling-spa")
                .output_config(ei::types::OutputConfig::Subtitling(
                    ei::types::SubtitlingConfig::builder()
                        .language("spa".into())
                        .build()
                        .unwrap(),
                ))
                .status(ei::types::OutputStatus::Enabled)
                .build()
                .unwrap(),
        )
        .outputs(
            ei::types::CreateOutput::builder()
                .name("event-clipping")
                .output_config(ei::types::OutputConfig::Clipping(
                    ei::types::ClippingConfig::builder().build(),
                ))
                .status(ei::types::OutputStatus::Enabled)
                .build()
                .unwrap(),
        )
        .send()
        .await
        .expect("create_feed failed");

    let feed_id = resp.id().to_string();
    info!(feed_id = %feed_id, "Created feed");

    client
        .associate_feed()
        .id(&feed_id)
        .associated_resource_name("ffmpeg-rust-ingest")
        .set_outputs(Some(vec![]))
        .send()
        .await
        .expect("associate_feed failed");
    info!(feed_id = %feed_id, "Feed associated");

    feed_id
}

/// PUT binary segment to data endpoint with SigV4 signing
async fn put_segment(
    http_client: &reqwest::Client,
    config: &aws_config::SdkConfig,
    endpoint: &str,
    feed_id: &str,
    region: &str,
    path: &str,
    data: Vec<u8>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let url = format!("https://{endpoint}/v1/feed/{feed_id}/input/0/media/{path}");

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

    let signable = SignableRequest::new(
        "PUT",
        &url,
        [("host", endpoint)].into_iter(),
        SignableBody::Bytes(&data),
    )?;

    let (instructions, _) = sign(signable, &signing_params.into())?.into_parts();

    let mut req = http::Request::builder()
        .method("PUT")
        .uri(&url)
        .body(())
        .unwrap();
    instructions.apply_to_request_http1x(&mut req);

    let reqwest_url = reqwest::Url::parse(&url)?;
    let mut final_req = http_client.put(reqwest_url).body(data);
    for (name, value) in req.headers() {
        final_req = final_req.header(name.as_str(), value.to_str()?);
    }

    let resp = final_req.send().await?;

    if !resp.status().is_success() {
        let body = resp.text().await?;
        error!(path = %path, response = %body, "PUT failed");
        return Err(format!("PUT {path} failed: {body}").into());
    }
    debug!(path = %path, "PUT success");
    Ok(())
}

/// Deliver segments with per-sequence synchronization
async fn deliver_segments(
    config: &aws_config::SdkConfig,
    endpoint: &str,
    feed_id: &str,
    region: &str,
    output_dir: &Path,
) {
    let http_client = reqwest::Client::new();
    let video_dir = output_dir.join("Streams(default-video.cmfv)");
    let audio_dir = output_dir.join("Streams(default-audio.cmfa)");

    info!("Sending init segments");
    let v_data = std::fs::read(video_dir.join("InitializationSegment")).unwrap();
    let a_data = std::fs::read(audio_dir.join("InitializationSegment")).unwrap();

    let (v_res, a_res) = tokio::join!(
        put_segment(
            &http_client,
            config,
            endpoint,
            feed_id,
            region,
            "Streams(default-video.cmfv)/InitializationSegment",
            v_data
        ),
        put_segment(
            &http_client,
            config,
            endpoint,
            feed_id,
            region,
            "Streams(default-audio.cmfa)/InitializationSegment",
            a_data
        ),
    );
    v_res.expect("video init failed");
    a_res.expect("audio init failed");

    let mut seq = 1u64;
    loop {
        let seg_name = format!("Segment({seq:09})");
        let v_path = video_dir.join(&seg_name);
        let a_path = audio_dir.join(&seg_name);

        if !v_path.exists() && !a_path.exists() {
            break;
        }

        let mut tasks = JoinSet::new();

        if v_path.exists() {
            let data = std::fs::read(&v_path).unwrap();
            let path = format!("Streams(default-video.cmfv)/{seg_name}");
            let c = http_client.clone();
            let cfg = config.clone();
            let ep = endpoint.to_string();
            let fid = feed_id.to_string();
            let r = region.to_string();
            tasks.spawn(async move { put_segment(&c, &cfg, &ep, &fid, &r, &path, data).await });
        }

        if a_path.exists() {
            let data = std::fs::read(&a_path).unwrap();
            let path = format!("Streams(default-audio.cmfa)/{seg_name}");
            let c = http_client.clone();
            let cfg = config.clone();
            let ep = endpoint.to_string();
            let fid = feed_id.to_string();
            let r = region.to_string();
            tasks.spawn(async move { put_segment(&c, &cfg, &ep, &fid, &r, &path, data).await });
        }

        while let Some(result) = tasks.join_next().await {
            result
                .unwrap()
                .unwrap_or_else(|e| panic!("segment {seq} delivery failed: {e}"));
        }

        info!(seq = seq, "Segment === ");
        seq += 1;
    }

    info!(total = seq - 1, "All segments delivered");
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("inference_ingest=info")
        .init();

    let cli = Cli::parse();
    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let client = ei::Client::new(&config);

    let feed_id = create_and_associate_feed(&client).await;

    info!(feed_id = %feed_id, "Delivering segments");
    deliver_segments(&config, &cli.endpoint, &feed_id, &cli.region, &cli.assets).await;

    info!(feed_id = %feed_id, "Done");
}
