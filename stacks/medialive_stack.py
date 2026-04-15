"""Stack: MediaLive channel with MP4 file input, linked to an Elemental Inference feed."""

import aws_cdk as cdk
from aws_cdk import (
    aws_iam as iam,
)
from aws_cdk import (
    aws_medialive as medialive,
)
from aws_cdk import (
    aws_s3 as s3,
)
from constructs import Construct


class MediaLiveStack(cdk.Stack):
    def __init__(
        self,
        scope: Construct,
        id: str,
        *,
        feed_arn: str,
        hls_bucket: s3.Bucket,
        **kwargs,
    ) -> None:
        super().__init__(scope, id, **kwargs)

        # --- IAM Role ---
        medialive_role = iam.Role(
            self,
            "MediaLiveRole",
            assumed_by=iam.ServicePrincipal("medialive.amazonaws.com"),
            inline_policies={
                "MediaLiveAccess": iam.PolicyDocument(
                    statements=[
                        iam.PolicyStatement(
                            actions=[
                                "medialive:*",
                                "elemental-inference:AssociateFeed",
                                "elemental-inference:DisassociateFeed",
                                "elemental-inference:GetFeed",
                                "logs:CreateLogGroup",
                                "logs:CreateLogStream",
                                "logs:PutLogEvents",
                                "logs:DescribeLogStreams",
                                "logs:DescribeLogGroups",
                            ],
                            resources=["*"],
                        ),
                    ],
                ),
            },
        )

        hls_bucket.grant_read_write(medialive_role)

        # --- MP4 File Input ---
        # Upload your .mp4 file to s3://<bucket>/input/
        # MediaLive will pull from this location.
        ml_input = medialive.CfnInput(
            self,
            "MediaLiveInput",
            name="medialive-mp4-input",
            type="MP4_FILE",
            sources=[
                {"url": f"s3ssl://{hls_bucket.bucket_name}/input/inference-output.mp4"},
            ],
        )

        # --- Channel ---
        medialive.CfnChannel(
            self,
            "MediaLiveChannel",
            name="medialive-inference-channel",
            channel_class="SINGLE_PIPELINE",
            role_arn=medialive_role.role_arn,
            log_level="INFO",
            input_attachments=[
                {
                    "inputId": ml_input.ref,
                    "inputAttachmentName": "primary-input",
                    "inputSettings": {
                        "sourceEndBehavior": "LOOP",
                        "audioSelectors": [
                            {
                                "name": "audio-primary",
                                "selectorSettings": {
                                    "audioTrackSelection": {
                                        "tracks": [{"track": 1}],
                                    },
                                },
                            },
                            {
                                "name": "audio-secondary",
                                "selectorSettings": {
                                    "audioTrackSelection": {
                                        "tracks": [{"track": 2}],
                                    },
                                },
                            },
                        ],
                    },
                },
            ],
            input_specification={
                "codec": "AVC",
                "resolution": "HD",
                "maximumBitrate": "MAX_10_MBPS",
            },
            inference_settings={
                "feedArn": feed_arn,
            },
            encoder_settings={
                "audioDescriptions": [
                    {
                        "name": "audio_primary",
                        "audioSelectorName": "audio-primary",
                        "codecSettings": {
                            "aacSettings": {
                                "bitrate": 128000,
                                "rawFormat": "NONE",
                                "spec": "MPEG4",
                            },
                        },
                    },
                    {
                        "name": "audio_secondary",
                        "audioSelectorName": "audio-secondary",
                        "codecSettings": {
                            "aacSettings": {
                                "bitrate": 128000,
                                "rawFormat": "NONE",
                                "spec": "MPEG4",
                            },
                        },
                    },
                ],
                "videoDescriptions": [
                    {
                        "name": "video_1",
                        "width": 1920,
                        "height": 1080,
                        "codecSettings": {
                            "h264Settings": {
                                "rateControlMode": "CBR",
                                "bitrate": 5000000,
                            },
                        },
                    },
                    {
                        "name": "video_smart_crop",
                        "width": 720,
                        "height": 1280,
                        "scalingBehavior": "SMART_CROP",
                        "codecSettings": {
                            "h264Settings": {
                                "rateControlMode": "CBR",
                                "bitrate": 3000000,
                            },
                        },
                    },
                ],
                "outputGroups": [
                    {
                        "name": "hls-landscape",
                        "outputGroupSettings": {
                            "hlsGroupSettings": {
                                "destination": {
                                    "destinationRefId": "hls-destination",
                                },
                            },
                        },
                        "outputs": [
                            {
                                "outputName": "hls-output",
                                "outputSettings": {
                                    "hlsOutputSettings": {
                                        "hlsSettings": {
                                            "standardHlsSettings": {
                                                "m3U8Settings": {},
                                            },
                                        },
                                    },
                                },
                                "videoDescriptionName": "video_1",
                                "audioDescriptionNames": [
                                    "audio_primary",
                                    "audio_secondary",
                                ],
                            },
                        ],
                    },
                    {
                        "name": "hls-vertical",
                        "outputGroupSettings": {
                            "hlsGroupSettings": {
                                "destination": {
                                    "destinationRefId": "vertical-destination",
                                },
                            },
                        },
                        "outputs": [
                            {
                                "outputName": "hls-smart-crop",
                                "outputSettings": {
                                    "hlsOutputSettings": {
                                        "hlsSettings": {
                                            "standardHlsSettings": {
                                                "m3U8Settings": {},
                                            },
                                        },
                                    },
                                },
                                "videoDescriptionName": "video_smart_crop",
                                "audioDescriptionNames": [
                                    "audio_primary",
                                ],
                            },
                        ],
                    },
                ],
                "timecodeConfig": {
                    "source": "SYSTEMCLOCK",
                },
            },
            destinations=[
                {
                    "id": "hls-destination",
                    "settings": [
                        {"url": f"s3://{hls_bucket.bucket_name}/hls/output"},
                    ],
                },
                {
                    "id": "vertical-destination",
                    "settings": [
                        {"url": f"s3://{hls_bucket.bucket_name}/vertical/output"},
                    ],
                },
            ],
        )
