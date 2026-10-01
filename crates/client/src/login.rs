use std::{fs, io::Write, path::PathBuf, thread, time::Duration};

use anyhow::{Context, bail};
use serde::Deserialize;

use crate::api::{self, Credentials};

fn credentials_path() -> anyhow::Result<PathBuf> {
    let base = match std::env::var_os("XDG_CONFIG_HOME") {
        Some(dir) => PathBuf::from(dir),
        None => PathBuf::from(std::env::var_os("HOME").context("$HOME is not set")?).join(".config"),
    };
    Ok(base.join("rusttype").join("credentials.json"))
}

pub fn load() -> Option<Credentials> {
    serde_json::from_slice(&fs::read(credentials_path().ok()?).ok()?).ok()
}

fn save(credentials: &Credentials) -> anyhow::Result<()> {
    let path = credentials_path()?;
    fs::create_dir_all(path.parent().context("config dir")?)?;
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(&path)?.write_all(&serde_json::to_vec_pretty(credentials)?)?;
    Ok(())
}

pub fn logout() -> anyhow::Result<()> {
    match fs::remove_file(credentials_path()?) {
        Ok(()) => println!("logged out"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => println!("not logged in"),
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

#[derive(Deserialize)]
struct DeviceCode {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: u64,
}

#[derive(Deserialize)]
struct TokenPoll {
    access_token: Option<String>,
    error: Option<String>,
    interval: Option<u64>,
}

/// GitHub device flow: no browser redirect or local web server needed, so it
/// works over any terminal.
pub fn login(server: &str) -> anyhow::Result<()> {
    let client_id = api::github_client_id(server)?;
    let agent = api::agent();
    let mut response = agent
        .post("https://github.com/login/device/code")
        .header("Accept", "application/json")
        .send_form([("client_id", client_id.as_str())])?;
    if !response.status().is_success() {
        bail!("GitHub refused the device code request ({})", response.status());
    }
    let device: DeviceCode = response.body_mut().read_json()?;

    println!("Open  {}  and enter the code:\n\n    {}\n", device.verification_uri, device.user_code);
    println!("Waiting for you to authorize (expires in {} min)...", device.expires_in / 60);

    let mut interval = device.interval.max(1);
    let mut waited = 0;
    let access_token = loop {
        thread::sleep(Duration::from_secs(interval));
        waited += interval;
        if waited > device.expires_in {
            bail!("the code expired; run `rusttype login` again");
        }
        let poll: TokenPoll = agent
            .post("https://github.com/login/oauth/access_token")
            .header("Accept", "application/json")
            .send_form([
                ("client_id", client_id.as_str()),
                ("device_code", device.device_code.as_str()),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])?
            .body_mut()
            .read_json()?;
        match (poll.access_token, poll.error.as_deref()) {
            (Some(token), _) => break token,
            (None, Some("authorization_pending")) => {}
            (None, Some("slow_down")) => interval = poll.interval.unwrap_or(interval + 5),
            (None, Some("access_denied")) => bail!("authorization was denied"),
            (None, Some("expired_token")) => bail!("the code expired; run `rusttype login` again"),
            (None, other) => bail!("unexpected response from GitHub: {}", other.unwrap_or("none")),
        }
    };

    let credentials = api::exchange_github_token(server, &access_token)?;
    save(&credentials)?;
    println!("Logged in as @{}. Scores will be submitted automatically.", credentials.login);
    Ok(())
}
