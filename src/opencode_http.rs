use std::{
    io::{Read, Write},
    net::TcpStream,
    time::Duration,
};

use crate::adapters::CommandPlan;

pub fn serve_plan(hostname: &str, port: u16) -> CommandPlan {
    CommandPlan {
        adapter: "opencode".to_string(),
        program: "opencode".to_string(),
        args: vec![
            "serve".to_string(),
            "--hostname".to_string(),
            hostname.to_string(),
            "--port".to_string(),
            port.to_string(),
        ],
    }
}

pub fn openapi_url(base_url: &str) -> String {
    format!("{}/doc", base_url.trim_end_matches('/'))
}

pub fn fetch_openapi(base_url: &str) -> crate::Result<String> {
    let target = HttpTarget::parse(&openapi_url(base_url))?;
    let mut stream = TcpStream::connect((target.host.as_str(), target.port))?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.write_all(
        format!(
            "GET {} HTTP/1.1\r\nHost: {}\r\nAccept: application/json\r\nConnection: close\r\n\r\n",
            target.path, target.host
        )
        .as_bytes(),
    )?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    split_http_body(&response)
}

fn split_http_body(response: &str) -> crate::Result<String> {
    let (head, body) = response
        .split_once("\r\n\r\n")
        .ok_or_else(|| anyhow::anyhow!("invalid HTTP response"))?;
    if !head.starts_with("HTTP/1.1 200") && !head.starts_with("HTTP/1.0 200") {
        anyhow::bail!(
            "opencode OpenAPI request failed: {}",
            head.lines().next().unwrap_or(head)
        );
    }
    Ok(body.to_string())
}

struct HttpTarget {
    host: String,
    port: u16,
    path: String,
}

impl HttpTarget {
    fn parse(url: &str) -> crate::Result<Self> {
        let rest = url
            .strip_prefix("http://")
            .ok_or_else(|| anyhow::anyhow!("only http:// opencode server URLs are supported"))?;
        let (host_port, path) = rest.split_once('/').unwrap_or((rest, ""));
        let (host, port) = match host_port.split_once(':') {
            Some((host, port)) => (host, port.parse()?),
            None => (host_port, 80),
        };
        if host.is_empty() {
            anyhow::bail!("missing host in opencode server URL");
        }
        Ok(Self {
            host: host.to_string(),
            port,
            path: format!("/{path}"),
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn builds_serve_plan_and_openapi_url() {
        let plan = super::serve_plan("127.0.0.1", 4096);
        assert_eq!(plan.program, "opencode");
        assert_eq!(
            plan.args,
            ["serve", "--hostname", "127.0.0.1", "--port", "4096"]
        );
        assert_eq!(
            super::openapi_url("http://127.0.0.1:4096/"),
            "http://127.0.0.1:4096/doc"
        );
    }

    #[test]
    fn parses_http_url() {
        let target = super::HttpTarget::parse("http://127.0.0.1:4096/doc").unwrap();
        assert_eq!(target.host, "127.0.0.1");
        assert_eq!(target.port, 4096);
        assert_eq!(target.path, "/doc");
    }
}
