//! Signing in to GitHub in the browser, by a code: the app asks GitHub for a
//! short code, the user types it in on github.com, and the app asks GitHub
//! until it says yes and gives an access token (OAuth's device flow). No
//! password passes through the app, and it holds no secret of its own: the
//! client id below only says which app is asking.

use anyhow::{Result, anyhow, bail};
use serde_json::Value;
use std::time::Duration;

/// XC Circuit's OAuth app on GitHub. Not a secret.
pub const CLIENT_ID: &str = "Ov23liXTC1Q579y0vIcI";
/// Where GitHub's sign-in lives.
pub const GITHUB: &str = "https://github.com";
/// What the app asks for: reading and writing the user's repositories.
const SCOPE: &str = "repo";

/// A sign-in under way: what to show the user, and what to ask GitHub with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Code {
    /// The code the user types in, such as `WDJB-MJHT`.
    pub user_code: String,
    /// The page to type it in on.
    pub page: String,
    /// The app's side of it: never shown.
    device_code: String,
    /// How long to wait between two questions.
    pub wait: Duration,
    /// How long the code is good for.
    pub good_for: Duration,
}

/// GitHub's answer to "has the user said yes?".
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Answer {
    /// Not yet: ask again after the wait.
    NotYet,
    /// Asked too often: wait this long from now on.
    Slower(Duration),
    /// Yes: the access token.
    Token(String),
    /// The user said no.
    Refused,
    /// The code ran out: start again.
    TooLate,
}

fn post(url: &str, form: &[(&str, &str)]) -> Result<Value> {
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(30))).http_status_as_error(false).build().into();
    let mut response = agent.post(url).header("Accept", "application/json").header("User-Agent", "XC-Circuit").send_form(form.iter().copied()).map_err(|e| anyhow!("Couldn't reach GitHub: {e}"))?;
    let text = response.body_mut().read_to_string().unwrap_or_default();
    serde_json::from_str(&text).map_err(|_| anyhow!("GitHub gave an answer that can't be read ({}).", response.status()))
}

/// Starts a sign-in. `site` is [`GITHUB`] (another address in tests).
pub fn start(site: &str, client_id: &str) -> Result<Code> {
    let v = post(&format!("{site}/login/device/code"), &[("client_id", client_id), ("scope", SCOPE)])?;
    let text = |key: &str| v[key].as_str().map(str::to_string).filter(|s| !s.is_empty());
    let (Some(device_code), Some(user_code)) = (text("device_code"), text("user_code")) else {
        bail!("GitHub didn't start the sign-in: {}", text("error_description").or(text("error")).unwrap_or_else(|| "no code came back".into()));
    };
    Ok(Code {
        user_code,
        page: text("verification_uri").unwrap_or_else(|| format!("{site}/login/device")),
        device_code,
        wait: Duration::from_secs(v["interval"].as_u64().unwrap_or(5).clamp(1, 60)),
        good_for: Duration::from_secs(v["expires_in"].as_u64().unwrap_or(900)),
    })
}

/// Asks once whether the user has typed the code in.
pub fn ask(site: &str, client_id: &str, code: &Code) -> Result<Answer> {
    let v = post(&format!("{site}/login/oauth/access_token"), &[("client_id", client_id), ("device_code", &code.device_code), ("grant_type", "urn:ietf:params:oauth:grant-type:device_code")])?;
    if let Some(token) = v["access_token"].as_str().filter(|t| !t.is_empty()) {
        return Ok(Answer::Token(token.to_string()));
    }
    Ok(match v["error"].as_str().unwrap_or_default() {
        "authorization_pending" => Answer::NotYet,
        "slow_down" => Answer::Slower(Duration::from_secs(v["interval"].as_u64().unwrap_or(code.wait.as_secs() + 5).clamp(1, 120))),
        "access_denied" => Answer::Refused,
        "expired_token" => Answer::TooLate,
        other => bail!("GitHub didn't finish the sign-in: {}", v["error_description"].as_str().unwrap_or(if other.is_empty() { "no token came back" } else { other })),
    })
}

/// The name of the account a token belongs to, to show who is signed in.
/// `api` is `https://api.github.com`.
pub fn account(api: &str, token: &str) -> Result<String> {
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(30))).http_status_as_error(false).build().into();
    let mut response = agent.get(&format!("{api}/user")).header("Accept", "application/vnd.github+json").header("User-Agent", "XC-Circuit").header("Authorization", &format!("Bearer {token}")).call().map_err(|e| anyhow!("Couldn't reach GitHub: {e}"))?;
    let v: Value = serde_json::from_str(&response.body_mut().read_to_string().unwrap_or_default()).unwrap_or_default();
    v["login"].as_str().map(str::to_string).ok_or_else(|| anyhow!("GitHub didn't accept the sign-in ({}).", response.status()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// A stand-in for GitHub: answers each request with the next body, and
    /// gives back what was asked.
    fn stand_in(answers: Vec<&'static str>) -> (String, std::thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let site = format!("http://{}", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let mut asked = Vec::new();
            for body in answers {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = Vec::new();
                let mut buf = [0u8; 4096];
                loop {
                    let n = stream.read(&mut buf).unwrap();
                    request.extend_from_slice(&buf[..n]);
                    let text = String::from_utf8_lossy(&request).to_string();
                    let Some(head_end) = text.find("\r\n\r\n") else { continue };
                    let length = text.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap())).unwrap_or(0);
                    if n == 0 || request.len() >= head_end + 4 + length {
                        break;
                    }
                }
                asked.push(String::from_utf8_lossy(&request).to_string());
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            }
            asked
        });
        (site, handle)
    }

    #[test]
    fn signs_in_by_a_code() {
        let (site, server) = stand_in(vec![
            r#"{"device_code":"dev-1","user_code":"WDJB-MJHT","verification_uri":"https://github.com/login/device","expires_in":900,"interval":5}"#,
            r#"{"error":"authorization_pending"}"#,
            r#"{"error":"slow_down","interval":10}"#,
            r#"{"access_token":"gho_test","token_type":"bearer","scope":"repo"}"#,
            r#"{"error":"access_denied"}"#,
            r#"{"error":"expired_token"}"#,
            r#"{"error":"incorrect_client_credentials","error_description":"The client_id is wrong."}"#,
            r#"{"error":"device_flow_disabled","error_description":"Device flow is off."}"#,
            r#"{"login":"octocat"}"#,
        ]);
        let code = start(&site, "client-1").unwrap();
        assert_eq!((code.user_code.as_str(), code.page.as_str(), code.wait, code.good_for), ("WDJB-MJHT", "https://github.com/login/device", Duration::from_secs(5), Duration::from_secs(900)));
        assert_eq!(ask(&site, "client-1", &code).unwrap(), Answer::NotYet);
        assert_eq!(ask(&site, "client-1", &code).unwrap(), Answer::Slower(Duration::from_secs(10)));
        assert_eq!(ask(&site, "client-1", &code).unwrap(), Answer::Token("gho_test".into()));
        assert_eq!(ask(&site, "client-1", &code).unwrap(), Answer::Refused);
        assert_eq!(ask(&site, "client-1", &code).unwrap(), Answer::TooLate);
        assert!(ask(&site, "client-1", &code).unwrap_err().to_string().contains("The client_id is wrong."));
        assert!(start(&site, "client-1").unwrap_err().to_string().contains("Device flow is off."));
        assert_eq!(account(&site, "gho_test").unwrap(), "octocat");
        let asked = server.join().unwrap();
        assert!(asked[0].starts_with("POST /login/device/code") && asked[0].contains("client_id=client-1") && asked[0].contains("scope=repo"), "{}", asked[0]);
        assert!(asked[1].starts_with("POST /login/oauth/access_token") && asked[1].contains("device_code=dev-1") && asked[1].contains("grant-type%3Adevice_code"), "{}", asked[1]);
        assert!(asked[8].starts_with("GET /user") && asked[8].to_ascii_lowercase().contains("authorization: bearer gho_test"), "{}", asked[8]);
    }
}
