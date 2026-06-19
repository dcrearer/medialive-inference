# MediaLive Inference

Real-time video inference pipeline using AWS Elemental Inference for smart cropping, subtitling, and event clipping.

## Architecture

This project supports two ingest paths into Elemental Inference:

1. **MediaLive** — AWS Elemental MediaLive channel with native `inferenceSettings` integration
2. **FFmpeg/CMAF** — Direct CMAF ingest via the Elemental Inference PutMedia API (no MediaLive required)

## Project Structure

```
├── stacks/                     # CDK infrastructure (Python)
│   ├── inference_stack.py      # Elemental Inference feed
│   ├── medialive_stack.py      # MediaLive channel
│   ├── storage_stack.py        # S3 buckets
│   └── event_processing_stack.py # EventBridge + Lambda
├── lambda/
│   └── event_logger/           # Rust Lambda for inference events
├── tools/
│   ├── inference-ingest/       # Rust CLI — deliver CMAF to Inference
│   └── inference-metadata/     # Rust CLI — query metadata from Inference
└── cmaf-test/                  # CMAF ingest documentation
```

## CDK Stacks

Deploy infrastructure with:

```bash
pip install -r requirements.txt
cdk deploy --all
```

| Stack | Purpose |
|-------|---------|
| StorageStack | S3 buckets for media assets |
| InferenceStack | Elemental Inference feed (smart crop, subtitles, event clipping) |
| MediaLiveStack | MediaLive channel linked to the Inference feed |
| EventProcessingStack | EventBridge rule → Rust Lambda for inference events |

## Tools

### inference-ingest

Delivers pre-segmented CMAF assets to a fresh Elemental Inference feed.

```bash
cd tools/inference-ingest
cargo run -- --endpoint <data-endpoint>
```

- Creates a new feed with smart-cropping, subtitling (Spanish), and event-clipping outputs
- Associates the feed
- Sends init + media segments with per-sequence synchronization
- Prints the feed ID for metadata queries

### inference-metadata

Queries metadata from an Elemental Inference feed.

```bash
cd tools/inference-metadata
cargo run -- --feed-id <id> --endpoint <data-endpoint> --output-type subtitles --end 60000
cargo run -- --feed-id <id> --endpoint <data-endpoint> --output-type smart-crop --end 5000
```

Subtitles are automatically parsed into timestamped text. Smart crop returns JSON with per-frame centerpoints.

## CMAF Ingest Requirements

- Video: H.264 Main profile, 1280x720, keyframe every 1s
- Audio: AAC, 48kHz stereo
- Container: CMAF Ingest (Interface-1 v1.2), 1-second segments
- Delivery: SigV4-signed HTTP PUT to the feed's data endpoint

See `cmaf-test/README.md` for the full step-by-step process.

## Prerequisites

- Python 3.11+, AWS CDK CLI
- Rust toolchain (for tools and Lambda)
- FFmpeg (for CMAF segmentation)
- `awscurl` (for manual testing)
- AWS credentials with Elemental Inference access
