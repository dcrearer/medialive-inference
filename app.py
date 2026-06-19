#!/usr/bin/env python3
"""CDK app: MediaLive channel with Elemental Inference for event clipping and smart cropping."""

import aws_cdk as cdk

from stacks import EventProcessingStack, InferenceStack, MediaLiveStack, StorageStack

app = cdk.App()

# Deploy inference feed first — MediaLive stack depends on its ARN
inference = InferenceStack(app, "InferenceStack")

# S3 bucket for HLS output
storage = StorageStack(app, "StorageStack")

# MediaLive channel linked to the inference feed and output bucket
MediaLiveStack(
    app, "MediaLiveStack", feed_arn=inference.feed_arn, hls_bucket=storage.hls_bucket
)

# EventBridge rule + Lambda logger for Elemental Inference events
EventProcessingStack(app, "EventProcessingStack")

app.synth()
