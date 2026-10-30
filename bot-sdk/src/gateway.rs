//! The gateway connection (protocol in [`protocol::gateway`]).
//!
//! One task per bot: connect → `hello` → `identify` (or `resume` with the
//! last sequence number) → dispatches and heartbeats. After a drop it
//! reconnects with jittered backoff and resumes, so missed events are
//! replayed from the server's buffer. When the server refuses the resume it
//! identifies again; the new `ready` resets the cache and the guild
//! snapshots that follow fill it again.

use std::sync::Arc;
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use protocol::gateway::close_code;
use protocol::{ApiError, Event, GatewayMessage, StatusSetting, Token};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::time::{Instant, sleep, sleep_until, timeout};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::http::header::USER_AGENT;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::{self, Message as WsMessage};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use crate::client::{BotEvent, Shared};
use crate::error::Error;

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Commands from [`Bot`](crate::Bot) handles to the gateway task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Command {
    /// Close the connection cleanly and stop.
    Shutdown,
    /// Set the bot's status (online, idle, do not disturb, invisible).
    Status(StatusSetting),
}

/// Reconnect and connect timing.
#[derive(Debug, Clone)]
pub(crate) struct Timing {
    pub connect_timeout: Duration,
    pub reconnect_min: Duration,
    pub reconnect_max: Duration,
}

/// What to do after the server closed the connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CloseAction {
    /// Reconnect with backoff and resume.
    Resume,
    /// Reconnect after at least this long, then resume.
    ResumeAfter(Duration),
    /// Reconnect and identify again (the session is gone).
    Reidentify,
    /// Stop for good.
    Fatal(Error),
}

/// Decides from the close code (`None`: no close frame, a network drop) and
/// the reason (an `ApiError` body for authentication failures).
pub(crate) fn close_action(code: Option<u16>, reason: &str) -> CloseAction {
    let Some(code) = code else {
        return CloseAction::Resume;
    };
    match code {
        close_code::SESSION_TIMED_OUT => CloseAction::Resume,
        close_code::UNKNOWN_ERROR => CloseAction::ResumeAfter(Duration::from_secs(1)),
        close_code::RATE_LIMITED => CloseAction::ResumeAfter(Duration::from_secs(5)),
        close_code::NOT_AUTHENTICATED => CloseAction::Reidentify,
        close_code::AUTHENTICATION_FAILED => CloseAction::Fatal(Error::Api(
            serde_json::from_str::<ApiError>(reason).unwrap_or(ApiError::Unauthorized),
        )),
        code if (4000..=4999).contains(&code) => CloseAction::Fatal(Error::GatewayClosed(code)),
        // Normal, going away, server restart, ...: still resumable.
        _ => CloseAction::Resume,
    }
}

/// The delay for reconnect attempt `attempt` (0-based): doubling from
/// `min` up to `max`, half fixed and half random (`unit` in `[0, 1)`).
pub(crate) fn backoff(min: Duration, max: Duration, attempt: u32, unit: f64) -> Duration {
    let base = min.saturating_mul(1 << attempt.min(20)).min(max.max(min));
    base / 2 + base.mul_f64(unit.clamp(0.0, 1.0) / 2.0)
}

#[derive(Debug, Clone)]
struct ResumeState {
    session_id: String,
    seq: u64,
}

enum Outcome {
    Shutdown,
    Fatal(Error),
    Retry {
        min_delay: Duration,
        reidentify: bool,
    },
    ReconnectNow,
}

impl Outcome {
    const fn retry() -> Self {
        Self::Retry {
            min_delay: Duration::ZERO,
            reidentify: false,
        }
    }

    fn from_close(frame: Option<&CloseFrame>) -> Self {
        let code = frame.map(|frame| u16::from(frame.code));
        let reason = frame.map_or("", |frame| frame.reason.as_str());
        match close_action(code, reason) {
            CloseAction::Resume => Self::retry(),
            CloseAction::ResumeAfter(min_delay) => Self::Retry {
                min_delay,
                reidentify: false,
            },
            CloseAction::Reidentify => Self::Retry {
                min_delay: Duration::ZERO,
                reidentify: true,
            },
            CloseAction::Fatal(error) => Self::Fatal(error),
        }
    }
}

pub(crate) struct GatewayTask {
    shared: Arc<Shared>,
    url: String,
    token: Token,
    timing: Timing,
    commands: mpsc::UnboundedReceiver<Command>,
    resume: Option<ResumeState>,
    attempt: u32,
    /// The status to (re)send after connecting.
    status: Option<StatusSetting>,
}

impl GatewayTask {
    pub(crate) fn new(
        shared: Arc<Shared>,
        url: String,
        token: Token,
        timing: Timing,
        commands: mpsc::UnboundedReceiver<Command>,
    ) -> Self {
        Self {
            shared,
            url,
            token,
            timing,
            commands,
            resume: None,
            attempt: 0,
            status: None,
        }
    }

    pub(crate) async fn run(mut self) {
        loop {
            let delay = match self.connect().await {
                Outcome::Shutdown => {
                    self.shared.closed(None);
                    return;
                }
                Outcome::Fatal(error) => {
                    log::warn!("gateway: session ended: {error}");
                    self.shared.closed(Some(error));
                    return;
                }
                Outcome::ReconnectNow => Duration::ZERO,
                Outcome::Retry {
                    min_delay,
                    reidentify,
                } => {
                    if reidentify {
                        self.resume = None;
                    }
                    let delay = backoff(
                        self.timing.reconnect_min,
                        self.timing.reconnect_max,
                        self.attempt,
                        rand::random::<f64>(),
                    );
                    self.attempt = self.attempt.saturating_add(1);
                    delay.max(min_delay)
                }
            };
            self.shared.emit(BotEvent::Disconnected);
            log::debug!("gateway: reconnecting in {delay:?}");
            let wake = Instant::now() + delay;
            loop {
                tokio::select! {
                    () = sleep_until(wake) => break,
                    command = self.commands.recv() => match command {
                        Some(Command::Status(status)) => self.status = Some(status),
                        None | Some(Command::Shutdown) => {
                            self.shared.closed(None);
                            return;
                        }
                    },
                }
            }
        }
    }

    async fn connect(&mut self) -> Outcome {
        let mut request = match self.url.as_str().into_client_request() {
            Ok(request) => request,
            Err(error) => return Outcome::Fatal(Error::InvalidUrl(error.to_string())),
        };
        request.headers_mut().insert(
            USER_AGENT,
            HeaderValue::from_static(concat!("era-bot-sdk/", env!("CARGO_PKG_VERSION"))),
        );
        let connecting = timeout(
            self.timing.connect_timeout,
            tokio_tungstenite::connect_async(request),
        );
        tokio::pin!(connecting);
        let connected = loop {
            tokio::select! {
                result = &mut connecting => break result,
                command = self.commands.recv() => match command {
                    Some(Command::Status(status)) => self.status = Some(status),
                    None | Some(Command::Shutdown) => return Outcome::Shutdown,
                },
            }
        };
        let mut socket = match connected {
            Ok(Ok((socket, _))) => socket,
            Ok(Err(error)) => {
                log::debug!("gateway: connect failed: {}", describe(&error));
                return Outcome::retry();
            }
            Err(_) => {
                log::debug!("gateway: connect timed out");
                return Outcome::retry();
            }
        };
        let interval = match timeout(self.timing.connect_timeout, next_message(&mut socket)).await {
            Ok(Ok(GatewayMessage::Hello {
                heartbeat_interval_ms,
            })) => Duration::from_millis(heartbeat_interval_ms.max(100)),
            Ok(Ok(other)) => {
                log::warn!("gateway: expected hello, got {other:?}");
                return Outcome::retry();
            }
            Ok(Err(outcome)) => return outcome,
            Err(_) => return Outcome::retry(),
        };
        let start = match &self.resume {
            Some(resume) => GatewayMessage::Resume {
                token: self.token.clone(),
                session_id: resume.session_id.clone(),
                last_seq: resume.seq,
            },
            None => GatewayMessage::Identify {
                token: self.token.clone(),
                idle: false,
                // Bots show no device platform (§8.4).
                client: None,
            },
        };
        if send(&mut socket, &start).await.is_err() {
            return Outcome::retry();
        }
        if let Some(status) = self.status
            && send(
                &mut socket,
                &GatewayMessage::UpdatePresence {
                    idle: None,
                    status: Some(status),
                },
            )
            .await
            .is_err()
        {
            return Outcome::retry();
        }
        self.serve(&mut socket, interval).await
    }

    async fn serve(&mut self, socket: &mut Socket, interval: Duration) -> Outcome {
        // The first beat after a random fraction of the interval, so bots
        // reconnecting together do not beat together.
        let mut next_beat = Instant::now() + interval.mul_f64(rand::random::<f64>());
        let mut awaiting_ack = false;
        loop {
            tokio::select! {
                command = self.commands.recv() => match command {
                    Some(Command::Status(status)) => {
                        self.status = Some(status);
                        let update = GatewayMessage::UpdatePresence { idle: None, status: Some(status) };
                        if send(socket, &update).await.is_err() {
                            return Outcome::retry();
                        }
                    }
                    None | Some(Command::Shutdown) => {
                        let _ = socket
                            .close(Some(CloseFrame {
                                code: CloseCode::Normal,
                                reason: "bot shutting down".into(),
                            }))
                            .await;
                        return Outcome::Shutdown;
                    }
                },
                () = sleep_until(next_beat) => {
                    if awaiting_ack {
                        log::info!("gateway: heartbeat not acknowledged, reconnecting");
                        return Outcome::retry();
                    }
                    let last_seq = self.resume.as_ref().map_or(0, |resume| resume.seq);
                    if send(socket, &GatewayMessage::Heartbeat { last_seq }).await.is_err() {
                        return Outcome::retry();
                    }
                    awaiting_ack = true;
                    next_beat = Instant::now() + interval;
                }
                frame = socket.next() => {
                    let message = match frame {
                        Some(Ok(WsMessage::Text(text))) => {
                            match serde_json::from_str::<GatewayMessage>(text.as_str()) {
                                Ok(message) => message,
                                Err(error) => {
                                    log::warn!("gateway: ignoring undecodable frame: {error}");
                                    continue;
                                }
                            }
                        }
                        Some(Ok(WsMessage::Close(frame))) => {
                            log::info!("gateway: closed by server: {frame:?}");
                            return Outcome::from_close(frame.as_ref());
                        }
                        Some(Ok(_)) => continue,
                        Some(Err(error)) => {
                            log::info!("gateway: connection lost: {}", describe(&error));
                            return Outcome::retry();
                        }
                        None => return Outcome::retry(),
                    };
                    match message {
                        GatewayMessage::HeartbeatAck => awaiting_ack = false,
                        GatewayMessage::Heartbeat { .. } => {
                            let last_seq = self.resume.as_ref().map_or(0, |resume| resume.seq);
                            if send(socket, &GatewayMessage::Heartbeat { last_seq }).await.is_err() {
                                return Outcome::retry();
                            }
                            awaiting_ack = true;
                            next_beat = Instant::now() + interval;
                        }
                        other => {
                            if let Some(outcome) = self.on_message(socket, other).await {
                                return outcome;
                            }
                        }
                    }
                }
            }
        }
    }

    async fn on_message(
        &mut self,
        socket: &mut Socket,
        message: GatewayMessage,
    ) -> Option<Outcome> {
        match message {
            GatewayMessage::Dispatch { seq, event } => {
                if let Some(resume) = &mut self.resume {
                    if seq <= resume.seq {
                        return None; // replayed twice
                    }
                    resume.seq = seq;
                }
                self.dispatch(event);
            }
            GatewayMessage::Ready {
                session_id,
                user,
                guilds,
                ..
            } => {
                self.resume = Some(ResumeState { session_id, seq: 0 });
                self.attempt = 0;
                self.shared.ready(user, guilds);
            }
            GatewayMessage::Resumed => {
                log::info!("gateway: resumed");
                self.attempt = 0;
                self.shared.emit(BotEvent::Resumed);
            }
            GatewayMessage::Reconnect => {
                log::info!("gateway: server asked to reconnect");
                let _ = socket
                    .close(Some(CloseFrame {
                        code: CloseCode::Away,
                        reason: "reconnecting".into(),
                    }))
                    .await;
                return Some(Outcome::ReconnectNow);
            }
            GatewayMessage::InvalidSession { resumable } => {
                if resumable && let Some(resume) = self.resume.clone() {
                    sleep(Duration::from_millis(1_000 + rand::random_range(0..2_000))).await;
                    let message = GatewayMessage::Resume {
                        token: self.token.clone(),
                        session_id: resume.session_id,
                        last_seq: resume.seq,
                    };
                    if send(socket, &message).await.is_err() {
                        return Some(Outcome::retry());
                    }
                } else {
                    log::info!("gateway: session invalid, identifying again");
                    self.resume = None;
                    let identify = GatewayMessage::Identify {
                        token: self.token.clone(),
                        idle: false,
                        // Bots show no device platform (§8.4).
                        client: None,
                    };
                    if send(socket, &identify).await.is_err() {
                        return Some(Outcome::retry());
                    }
                }
            }
            GatewayMessage::Hello { .. }
            | GatewayMessage::Identify { .. }
            | GatewayMessage::Resume { .. }
            | GatewayMessage::UpdatePresence { .. }
            | GatewayMessage::Heartbeat { .. }
            | GatewayMessage::HeartbeatAck => {}
        }
        None
    }

    fn dispatch(&self, event: Event) {
        self.shared.dispatch(event);
    }
}

async fn next_message(socket: &mut Socket) -> Result<GatewayMessage, Outcome> {
    loop {
        match socket.next().await {
            Some(Ok(WsMessage::Text(text))) => {
                if let Ok(message) = serde_json::from_str(text.as_str()) {
                    return Ok(message);
                }
            }
            Some(Ok(WsMessage::Close(frame))) => return Err(Outcome::from_close(frame.as_ref())),
            Some(Ok(_)) => {}
            Some(Err(_)) | None => return Err(Outcome::retry()),
        }
    }
}

async fn send(socket: &mut Socket, message: &GatewayMessage) -> Result<(), ()> {
    let text = serde_json::to_string(message).map_err(|error| {
        log::error!("gateway: encoding a frame failed: {error}");
    })?;
    socket
        .send(WsMessage::text(text))
        .await
        .map_err(|error| log::debug!("gateway: send failed: {}", describe(&error)))
}

/// An error description without URLs.
fn describe(error: &tungstenite::Error) -> String {
    match error {
        tungstenite::Error::Http(response) => format!("HTTP {}", response.status()),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_codes_map_to_actions() {
        assert_eq!(close_action(None, ""), CloseAction::Resume);
        assert_eq!(close_action(Some(1000), ""), CloseAction::Resume);
        assert_eq!(close_action(Some(1012), ""), CloseAction::Resume);
        assert_eq!(close_action(Some(4009), ""), CloseAction::Resume);
        assert_eq!(
            close_action(Some(4000), ""),
            CloseAction::ResumeAfter(Duration::from_secs(1))
        );
        assert_eq!(close_action(Some(4003), ""), CloseAction::Reidentify);
        assert_eq!(
            close_action(Some(4004), r#"{"code":40100,"message":"unauthorized"}"#),
            CloseAction::Fatal(Error::Api(ApiError::Unauthorized))
        );
        assert_eq!(
            close_action(Some(4012), ""),
            CloseAction::Fatal(Error::GatewayClosed(4012))
        );
    }

    #[test]
    fn backoff_doubles_with_jitter_and_caps() {
        let min = Duration::from_millis(500);
        let max = Duration::from_secs(30);
        assert_eq!(backoff(min, max, 0, 0.0), Duration::from_millis(250));
        assert_eq!(backoff(min, max, 0, 1.0), min);
        assert_eq!(backoff(min, max, 3, 1.0), Duration::from_secs(4));
        assert_eq!(backoff(min, max, 40, 1.0), max);
    }
}
