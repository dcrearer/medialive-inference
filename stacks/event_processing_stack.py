"""Stack: EventBridge rule for Elemental Inference events → Rust Lambda logger."""

import aws_cdk as cdk
from aws_cdk import (
    aws_events as events,
    aws_events_targets as targets,
    aws_lambda as _lambda,
)
from constructs import Construct


class EventProcessingStack(cdk.Stack):
    def __init__(self, scope: Construct, id: str, **kwargs) -> None:
        super().__init__(scope, id, **kwargs)

        fn = _lambda.Function(
            self, "InferenceEventLogger",
            runtime=_lambda.Runtime.PROVIDED_AL2023,
            architecture=_lambda.Architecture.ARM_64,
            handler="bootstrap",
            code=_lambda.Code.from_asset("lambda/event_logger/target/lambda/bootstrap"),
            timeout=cdk.Duration.seconds(10),
            memory_size=128,
        )

        rule = events.Rule(
            self, "InferenceEventRule",
            event_pattern=events.EventPattern(
                source=["aws.elemental-inference"],
            ),
        )
        rule.add_target(targets.LambdaFunction(fn))
