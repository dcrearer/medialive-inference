"""Stack: S3 bucket for HLS output, destroyed on cdk destroy."""

import aws_cdk as cdk
from aws_cdk import RemovalPolicy, aws_s3 as s3
from constructs import Construct


class StorageStack(cdk.Stack):
    def __init__(self, scope: Construct, id: str, **kwargs) -> None:
        super().__init__(scope, id, **kwargs)

        self.hls_bucket = s3.Bucket(
            self, "HlsBucket",
            removal_policy=RemovalPolicy.DESTROY,
            auto_delete_objects=True,
        )
