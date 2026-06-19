"""Stack: Elemental Inference feed with event clipping enabled."""

import aws_cdk as cdk
from aws_cdk import aws_elementalinference as elementalinference
from constructs import Construct


class InferenceStack(cdk.Stack):
    def __init__(self, scope: Construct, id: str, **kwargs) -> None:
        super().__init__(scope, id, **kwargs)

        # Create a feed with event clipping enabled.
        # Smart cropping is NOT defined here — MediaLive will manage the
        # cropping output automatically when the channel is linked via InferenceSettings.
        self.feed = elementalinference.CfnFeed(
            self,
            "InferenceFeed",
            name="medialive-inference-feed",
            outputs=[
                {
                    "name": "event-clipping",
                    "description": "Enables event clipping on this feed",
                    "status": "ENABLED",
                    "outputConfig": {
                        # callbackMetadata is included in the EventBridge event for each clip
                        "clipping": {
                            "callbackMetadata": "medialive-clip-event",
                        },
                    },
                },
                {
                    "name": "smart-subtitles",
                    "description": "TTML subtitles from audio in your source media",
                    "status": "ENABLED",
                    "outputConfig": {
                        "subtitling": {
                            "language": "eng-us",
                        },
                    },
                },
            ],
        )

        # Expose the feed ARN for cross-stack references
        self.feed_arn = self.feed.attr_arn
