//! Finite qualification of the existing experimental generated client in real WASM.
//! Production game methods are absent and are not qualified by these checks.

use crate::browser::{request, require, wait_for_cancellation};
use df_protocol::transport_fixture::{
    Sample, sample::Behavior, transport_fixture_client::TransportFixtureClient,
};
use df_rpc_bridge::BrowserChannel;
use futures::{
    FutureExt, SinkExt, StreamExt, TryStreamExt,
    channel::mpsc,
    future::LocalBoxFuture,
    stream::{self, Fuse},
};
use tonic::{Code, Status, Streaming};

type BrowserFixtureClient = TransportFixtureClient<BrowserChannel>;
type BrowserFixtureStream = Fuse<Streaming<Sample>>;

fn sample(sequence: u32) -> Sample {
    Sample {
        sequence,
        payload: vec![0, 255, 128, 0, 1, 127, 254],
        behavior: Behavior::Echo as i32,
        active_calls: 7,
        deadline_calls: 11,
    }
}

fn rpc_error(status: Status) -> String {
    status.to_string()
}

fn terminal_error(status: Status) -> Result<(), String> {
    require(
        status.code() == Code::InvalidArgument
            && status
                .metadata()
                .get("fixture-terminal")
                .is_some_and(|value| value == "observed"),
        "generated browser error code or terminal metadata changed",
    )
}

async fn finish(stream: &mut BrowserFixtureStream) -> Result<(), String> {
    require(
        stream.try_next().await.map_err(rpc_error)?.is_none(),
        "generated browser stream has an extra message",
    )?;
    // Latch terminal EOF before extracting Tonic's cached trailers. Reading
    // trailers consumes that cache; Fuse prevents repolling the completed body.
    let trailers = stream
        .get_mut()
        .trailers()
        .await
        .map_err(rpc_error)?
        .ok_or("generated browser OK trailers absent")?;
    require(
        trailers
            .get("fixture-terminal")
            .is_some_and(|value| value == "observed"),
        "generated browser OK terminal metadata changed",
    )?;
    require(
        stream.try_next().await.map_err(rpc_error)?.is_none(),
        "generated browser stream reopened after terminal EOF",
    )
}

async fn cancellation(client: &mut BrowserFixtureClient) -> Result<(), String> {
    let baseline = client
        .unary(request(Sample {
            payload: b"stats".to_vec(),
            ..sample(0)
        })?)
        .await
        .map_err(rpc_error)?
        .into_inner()
        .sequence;
    let expected = baseline
        .checked_add(1)
        .ok_or("synthetic cancellation counter exhausted")?;
    let mut output = client
        .server_stream(request(Sample {
            behavior: Behavior::Wait as i32,
            ..sample(51)
        })?)
        .await
        .map_err(rpc_error)?
        .into_inner()
        .fuse();
    require(
        output.try_next().await.map_err(rpc_error)?
            == Some(Sample {
                sequence: 0,
                behavior: Behavior::Wait as i32,
                ..sample(51)
            }),
        "generated browser cancellable stream did not begin",
    )?;
    drop(output);
    // Reuse the fixture's bounded cleanup observation; no new timer/task owner.
    wait_for_cancellation(client, expected).await?;
    require(
        client
            .unary(request(sample(52))?)
            .await
            .map_err(rpc_error)?
            .into_inner()
            == sample(52),
        "generated browser stream cancellation damaged the shared connection",
    )
}

// The generated client uses BrowserChannel's local service future. A Send future
// or native channel/executor is not required by this consuming boundary.
pub(super) fn verify(client: &mut BrowserFixtureClient) -> LocalBoxFuture<'_, Result<(), String>> {
    async move {
        let expected = sample(u32::MAX);
        let reply = client
            .unary(request(expected.clone())?)
            .await
            .map_err(rpc_error)?;
        require(
            reply
                .metadata()
                .get("fixture-server")
                .is_some_and(|value| value == "native-tonic"),
            "generated browser response metadata changed",
        )?;
        require(
            reply.into_inner() == expected,
            "generated browser unary lost protobuf fields or binary bytes",
        )?;

        let input = sample(19);
        let mut output = client
            .server_stream(request(input.clone())?)
            .await
            .map_err(rpc_error)?
            .into_inner()
            .fuse();
        for sequence in 0..3 {
            require(
                output.try_next().await.map_err(rpc_error)?
                    == Some(Sample {
                        sequence,
                        ..input.clone()
                    }),
                "generated browser server stream lost order or protobuf fields",
            )?;
        }
        finish(&mut output).await?;
        drop(output);

        let uploaded = client
            .client_stream(request(stream::iter([sample(1), sample(2), sample(3)]))?)
            .await
            .map_err(rpc_error)?
            .into_inner();
        require(
            uploaded
                == Sample {
                    sequence: 3,
                    payload: b"half-close observed".to_vec(),
                    behavior: Behavior::Echo as i32,
                    ..Sample::default()
                },
            "generated browser client stream lost its half-close receipt",
        )?;

        let (mut sender, receiver) = mpsc::channel(1);
        sender
            .send(sample(21))
            .await
            .map_err(|error| error.to_string())?;
        let mut output = client
            .bidi(request(receiver)?)
            .await
            .map_err(rpc_error)?
            .into_inner()
            .fuse();
        require(
            output.try_next().await.map_err(rpc_error)? == Some(sample(21)),
            "generated browser bidi did not echo before request half-close",
        )?;
        sender
            .send(sample(22))
            .await
            .map_err(|error| error.to_string())?;
        drop(sender);
        require(
            output.try_next().await.map_err(rpc_error)? == Some(sample(22)),
            "generated browser bidi lost the final protobuf message",
        )?;
        finish(&mut output).await?;
        drop(output);

        let rejected = Sample {
            behavior: Behavior::Reject as i32,
            ..sample(1)
        };
        terminal_error(
            client
                .unary(request(rejected.clone())?)
                .await
                .err()
                .ok_or("generated browser unary accepted rejected behavior")?,
        )?;
        terminal_error(
            client
                .server_stream(request(rejected.clone())?)
                .await
                .err()
                .ok_or("generated browser server stream accepted rejected behavior")?,
        )?;
        terminal_error(
            client
                .client_stream(request(stream::iter([rejected.clone()]))?)
                .await
                .err()
                .ok_or("generated browser client stream accepted rejected behavior")?,
        )?;
        let mut output = client
            .bidi(request(stream::iter([sample(31), rejected]))?)
            .await
            .map_err(rpc_error)?
            .into_inner()
            .fuse();
        require(
            output.try_next().await.map_err(rpc_error)? == Some(sample(31)),
            "generated browser bidi lost its message before rejection",
        )?;
        terminal_error(
            output
                .try_next()
                .await
                .err()
                .ok_or("generated browser bidi accepted rejected behavior")?,
        )?;
        require(
            output.try_next().await.map_err(rpc_error)?.is_none(),
            "generated browser bidi delivered data after rejection",
        )?;
        drop(output);

        for (direction, mut limited) in [
            ("encoding", client.clone().max_encoding_message_size(1)),
            ("decoding", client.clone().max_decoding_message_size(1)),
        ] {
            let status = limited
                .unary(request(sample(41))?)
                .await
                .err()
                .ok_or("generated browser client accepted a message beyond its limit")?;
            require(
                status.code() == Code::OutOfRange,
                &format!(
                    "generated browser client {direction} message limit expected OutOfRange; actual code={:?}; message={}; details_bytes={}",
                    status.code(),
                    status.message(),
                    status.details().len(),
                ),
            )?;
        }
        require(
            client
                .unary(request(sample(42))?)
                .await
                .map_err(rpc_error)?
                .into_inner()
                == sample(42),
            "generated browser client limit failure damaged the shared connection",
        )?;
        cancellation(client).await
    }
    .boxed_local()
}
