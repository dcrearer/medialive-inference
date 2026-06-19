# FFmpeg to Elemental Inference (CMAF Ingest)

## Requirements

- `awscurl` installed
- AWS credentials exported
- Video must be **1280x720** (1080p is not supported)
- Feed must be **associated** before accepting media

## Step 1: Create CMAF Assets

```bash
mkdir -p 'Streams(default-video.cmfv)' 'Streams(default-audio.cmfa)'

# Source (10-sec test pattern)
ffmpeg -y -f lavfi -i "testsrc2=duration=10:size=1280x720:rate=30" \
  -f lavfi -i "sine=frequency=440:duration=10:sample_rate=48000" \
  -c:v libx264 -profile:v main -pix_fmt yuv420p -c:a aac -ac 2 -ar 48000 \
  input.mp4

# Video segments
ffmpeg -y -i input.mp4 \
  -map 0:v:0 -c:v libx264 -profile:v main -pix_fmt yuv420p \
  -g 30 -keyint_min 30 -sc_threshold 0 \
  -force_key_frames 'expr:gte(t,n_forced*1)' \
  -f dash -seg_duration 1 -use_timeline 0 -use_template 1 -remove_at_exit 0 \
  -init_seg_name 'Streams(default-video.cmfv)/InitializationSegment' \
  -media_seg_name 'Streams(default-video.cmfv)/Segment($Number%09d$)' \
  video.mpd

# Audio segments
ffmpeg -y -i input.mp4 \
  -map 0:a:0 -c:a aac -ar 48000 -ac 2 \
  -f dash -seg_duration 1 -use_timeline 0 -use_template 1 -remove_at_exit 0 \
  -init_seg_name 'Streams(default-audio.cmfa)/InitializationSegment' \
  -media_seg_name 'Streams(default-audio.cmfa)/Segment($Number%09d$)' \
  audio.mpd
```

## Step 2: Associate the Feed

```bash
https://docs.aws.amazon.com/elemental-inference/latest/APIReference/API_AssociateFeed.html
```

```bash
awscurl --region us-east-1 --service elemental-inference -X POST \
  "https://elemental-inference.us-east-1.amazonaws.com/v1/feed/<feed-id>/associate" \
  -H "Content-Type: application/json" \
  -d '{"associatedResourceName": "ffmpeg-local-ingest", "outputs": []}'
```

## Step 3: Deliver Media

```bash
export ENDPOINT="<endpoint>"
export FEED_ID="<id>"
export REGION="<region>"

# Init segments
awscurl --region $REGION --service elemental-inference -X PUT --data-binary \
  -d '@Streams(default-video.cmfv)/InitializationSegment' \
  "https://${ENDPOINT}/v1/feed/${FEED_ID}/input/0/media/Streams(default-video.cmfv)/InitializationSegment"

awscurl --region $REGION --service elemental-inference -X PUT --data-binary \
  -d '@Streams(default-audio.cmfa)/InitializationSegment' \
  "https://${ENDPOINT}/v1/feed/${FEED_ID}/input/0/media/Streams(default-audio.cmfa)/InitializationSegment"

# Media segments (interleaved)
for i in $(seq -f "%09g" 1 10); do
  awscurl --region $REGION --service elemental-inference -X PUT --data-binary \
    -d "@Streams(default-video.cmfv)/Segment(${i})" \
    "https://${ENDPOINT}/v1/feed/${FEED_ID}/input/0/media/Streams(default-video.cmfv)/Segment(${i})"

  awscurl --region $REGION --service elemental-inference -X PUT --data-binary \
    -d "@Streams(default-audio.cmfa)/Segment(${i})" \
    "https://${ENDPOINT}/v1/feed/${FEED_ID}/input/0/media/Streams(default-audio.cmfa)/Segment(${i})"
done
```

## Step 4: Query Metadata

```bash
# Smart crop (first 2 seconds)
awscurl --service elemental-inference --region $REGION \
  -X POST "https://${ENDPOINT}/v1/feed/${FEED_ID}/input/0/metadata" \
  -H "Content-Type: application/json" \
  -d '{"outputName": "smart-cropping", "timeSpecification": {"ptsBased": {"startPts": 0, "endPts": 2000, "timescale": 1000}}, "parameters": {"smartCropping": {"frameRate": {"numerator": 30, "denominator": 1}}}}'

# Subtitles (first 3 seconds)
awscurl --service elemental-inference --region $REGION \
  -X POST "https://${ENDPOINT}/v1/feed/${FEED_ID}/input/0/metadata" \
  -H "Content-Type: application/json" \
  -d '{"outputName": "subtitling-eng", "timeSpecification": {"ptsBased": {"startPts": 0, "endPts": 3000, "timescale": 1000}}, "parameters": {"subtitling": {}}}'
```

## Findings

- Feed must be associated via `AssociateFeed` API before it accepts PutMedia calls
- Only 1280x720 resolution is accepted
- `awscurl` requires `--data-binary` flag for binary CMAF segments
- Subtitles query requires `"parameters": {"subtitling": {}}` in the body
- FFmpeg can't send `lmsg` end-of-stream — avoid querying PTS ranges near the end of content
- Successful PUT returns `{}`; errors return JSON with a `message` field
