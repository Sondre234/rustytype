use std::{
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

use russh::{
    Channel, ChannelId, CryptoVec, Pty,
    keys::{PrivateKey, PublicKey, ssh_key::{Algorithm, LineEnding, rand_core::OsRng}},
    server::{Auth, Config, Handler, Msg, Server, Session},
};
use rusttype_core::{Action, App, Key, draw};

use crate::{AppState, service};

const ENTER_SCREEN: &[u8] = b"\x1b[?1049h\x1b[?25l";
const LEAVE_SCREEN: &[u8] = b"\x1b[0m\x1b[?25h\x1b[?1049l";
const GUEST: &str = "guest";

fn host_key(path: &Path) -> anyhow::Result<PrivateKey> {
    if path.exists() {
        return Ok(PrivateKey::read_openssh_file(path)?);
    }
    let key = PrivateKey::random(&mut OsRng, Algorithm::Ed25519)?;
    key.write_openssh_file(path, LineEnding::LF)?;
    Ok(key)
}

pub async fn serve(state: Arc<AppState>, address: String, host_key_path: &Path) -> anyhow::Result<()> {
    let config = Config {
        keys: vec![host_key(host_key_path)?],
        inactivity_timeout: Some(Duration::from_secs(15 * 60)),
        auth_rejection_time: Duration::from_secs(2),
        auth_rejection_time_initial: Some(Duration::ZERO),
        max_auth_attempts: 6,
        nodelay: true,
        ..Default::default()
    };
    let mut server = SshServer { state };
    server.run_on_address(Arc::new(config), address.as_str()).await?;
    Ok(())
}

struct SshServer {
    state: Arc<AppState>,
}

impl Server for SshServer {
    type Handler = Connection;
    fn new_client(&mut self, _peer: Option<std::net::SocketAddr>) -> Connection {
        Connection { state: Arc::clone(&self.state), player: None, user: None, size: (80, 24) }
    }
}

/// Who authenticated. `user_id` is `None` for guests, whose scores are not kept.
struct Player {
    login: String,
    user_id: Option<i64>,
}

struct Connection {
    state: Arc<AppState>,
    user: Option<Player>,
    player: Option<(ChannelId, App)>,
    size: (u16, u16),
}

impl Connection {
    /// Accept only a key listed on the GitHub account named by the SSH username.
    async fn verify_github_key(&mut self, login: &str, offered: &PublicKey) -> bool {
        let Some((github_user, keys)) = self.state.github.identity_and_keys(login).await else {
            return false;
        };
        if !keys.iter().any(|key| key.key_data() == offered.key_data()) {
            return false;
        }
        let db = Arc::clone(&self.state.db);
        let (id, name) = (github_user.id, github_user.login.clone());
        let Ok(Ok(user)) = tokio::task::spawn_blocking(move || db.upsert_user(id, &name)).await else {
            return false;
        };
        self.user = Some(Player { login: user.login, user_id: Some(user.id) });
        true
    }

    fn render(&self, session: &mut Session) {
        let Some((channel, app)) = &self.player else { return };
        let mut frame = Vec::new();
        if draw(&mut frame, app, self.size.0, self.size.1).is_ok() {
            let _ = session.data(*channel, CryptoVec::from(frame));
        }
    }

    async fn apply(&mut self, action: Action, session: &mut Session) {
        let Some((channel, app)) = &mut self.player else { return };
        let channel = *channel;
        match action {
            Action::None => {}
            Action::Quit => {
                let _ = session.data(channel, CryptoVec::from(LEAVE_SCREEN.to_vec()));
                let _ = session.exit_status_request(channel, 0);
                let _ = session.eof(channel);
                let _ = session.close(channel);
                self.player = None;
            }
            Action::Submit(submission) => {
                app.status = Some(match self.user.as_ref().and_then(|player| player.user_id) {
                    Some(user_id) => match service::submit(&self.state.db, user_id, submission).await {
                        Ok(outcome) => outcome.describe(),
                        Err(reason) => format!("not saved: {reason}"),
                    },
                    None => format!("guest run, not saved. ssh <github-login>@{}", self.state.public_host),
                });
            }
            Action::ShowLeaderboard { snippet } => {
                let entries = service::leaderboard(&self.state.db, snippet.map(String::from), 15).await;
                app.show_leaderboard(snippet, entries);
            }
        }
    }
}

impl Handler for Connection {
    type Error = anyhow::Error;

    async fn auth_none(&mut self, user: &str) -> Result<Auth, Self::Error> {
        if user == GUEST {
            self.user = Some(Player { login: GUEST.into(), user_id: None });
            return Ok(Auth::Accept);
        }
        Ok(Auth::reject())
    }

    async fn auth_publickey_offered(&mut self, user: &str, key: &PublicKey) -> Result<Auth, Self::Error> {
        // Only let the client sign with keys that can possibly succeed, so it
        // moves on to its next key instead of failing after a signature.
        if user == GUEST || self.verify_github_key(user, key).await {
            Ok(Auth::Accept)
        } else {
            Ok(Auth::reject())
        }
    }

    async fn auth_publickey(&mut self, user: &str, key: &PublicKey) -> Result<Auth, Self::Error> {
        if user == GUEST {
            self.user = Some(Player { login: GUEST.into(), user_id: None });
            return Ok(Auth::Accept);
        }
        if self.verify_github_key(user, key).await {
            Ok(Auth::Accept)
        } else {
            Ok(Auth::reject())
        }
    }

    async fn channel_open_session(&mut self, channel: Channel<Msg>, _session: &mut Session) -> Result<bool, Self::Error> {
        if self.player.is_some() {
            return Ok(false);
        }
        self.player = Some((channel.id(), App::new()));
        Ok(true)
    }

    async fn pty_request(
        &mut self,
        channel: ChannelId,
        _term: &str,
        cols: u32,
        rows: u32,
        _px: u32,
        _py: u32,
        _modes: &[(Pty, u32)],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.size = (cols.clamp(20, 500) as u16, rows.clamp(10, 200) as u16);
        session.channel_success(channel)?;
        Ok(())
    }

    async fn shell_request(&mut self, channel: ChannelId, session: &mut Session) -> Result<(), Self::Error> {
        session.channel_success(channel)?;
        let identity = self.user.as_ref().map(|player| player.login.clone());
        if let Some((_, app)) = &mut self.player {
            app.identity = identity;
        }
        session.data(channel, CryptoVec::from(ENTER_SCREEN.to_vec()))?;
        self.render(session);
        Ok(())
    }

    async fn window_change_request(
        &mut self,
        _channel: ChannelId,
        cols: u32,
        rows: u32,
        _px: u32,
        _py: u32,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.size = (cols.clamp(20, 500) as u16, rows.clamp(10, 200) as u16);
        self.render(session);
        Ok(())
    }

    async fn data(&mut self, _channel: ChannelId, data: &[u8], session: &mut Session) -> Result<(), Self::Error> {
        let now = Instant::now();
        for key in parse_keys(data) {
            let Some((_, app)) = &mut self.player else { return Ok(()) };
            let action = app.handle_key(key, now);
            self.apply(action, session).await;
        }
        self.render(session);
        Ok(())
    }
}

/// Turn raw terminal bytes into game keys. Escape sequences (arrows, function
/// keys) are skipped; a lone ESC byte quits, like Esc in the local client.
pub fn parse_keys(bytes: &[u8]) -> Vec<Key> {
    let mut keys = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            0x1b if bytes.len() == 1 => keys.push(Key::Quit),
            0x1b => {
                // CSI (ESC [ ... final) or SS3 (ESC O x) or Alt-<key>: skip it whole.
                index += 1;
                if matches!(bytes.get(index), Some(b'[')) {
                    index += 1;
                    while index < bytes.len() && !(0x40..=0x7e).contains(&bytes[index]) {
                        index += 1;
                    }
                } else if matches!(bytes.get(index), Some(b'O')) {
                    index += 1;
                }
            }
            0x03 | 0x04 => keys.push(Key::Quit),
            0x09 => keys.push(Key::Tab),
            0x12 => keys.push(Key::Restart),
            0x7f | 0x08 => keys.push(Key::Backspace),
            b'\r' | b'\n' => keys.push(Key::Enter),
            byte @ 0x20..=0x7e => keys.push(Key::Char(byte as char)),
            _ => {}
        }
        index += 1;
    }
    keys
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_typing_and_editing_keys_map_to_game_keys() {
        assert_eq!(
            parse_keys(b"a(\x7f\r"),
            [Key::Char('a'), Key::Char('('), Key::Backspace, Key::Enter]
        );
        assert_eq!(parse_keys(b"\x03"), [Key::Quit]);
        assert_eq!(parse_keys(b"\x12"), [Key::Restart]);
        assert_eq!(parse_keys(b"\t"), [Key::Tab]);
        assert_eq!(parse_keys(b"\x1b"), [Key::Quit]);
    }

    #[test]
    fn escape_sequences_are_swallowed_not_typed() {
        // Up arrow, then 'x'; F5 (ESC [ 1 5 ~), then 'y'; SS3 right arrow.
        assert_eq!(parse_keys(b"\x1b[Ax"), [Key::Char('x')]);
        assert_eq!(parse_keys(b"\x1b[15~y"), [Key::Char('y')]);
        assert_eq!(parse_keys(b"\x1bOC"), []);
    }
}
