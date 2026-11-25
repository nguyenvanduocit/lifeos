use plugin_sdk::{Call, Definition, Effect, Error, Tool};
use schemars::JsonSchema;
use serde::Deserialize;
use std::time::Duration;

#[derive(Deserialize, JsonSchema)]
struct Empty {}
#[derive(Deserialize)]
struct Token {
    access_token: String,
}
#[derive(Deserialize)]
struct Report {
    balances: Vec<Balance>,
    last_refresh_time: Option<String>,
}
#[derive(Deserialize)]
struct Balance {
    total_balance: Money,
    available_balance: Option<Money>,
}
#[derive(Deserialize)]
struct Money {
    currency_code: String,
    value: String,
}

fn credentials(call: &Call) -> Result<(&str, &str, &str, &str), Error> {
    let env = call.var("PAYPAL_ENV");
    let (mode, base) = match env {
        "" | "sandbox" => ("Sandbox", "https://api-m.sandbox.paypal.com"),
        "live" => ("Live", "https://api-m.paypal.com"),
        _ => return Err("PAYPAL_ENV phải là sandbox hoặc live.".into()),
    };
    let id = call.var("PAYPAL_CLIENT_ID");
    let secret = call.var("PAYPAL_CLIENT_SECRET");
    if id.is_empty() || secret.is_empty() {
        return Err(format!(
            "{mode} · Mở Integrations → Connections → PayPal để thiết lập kết nối."
        )
        .into());
    }
    Ok((base, mode, id, secret))
}

async fn balances(call: Call, _: Empty) -> Result<String, Error> {
    let (base, mode, id, secret) = credentials(&call)?;
    fetch_balances(base, mode, id, secret).await
}

async fn authenticate(
    base: &str,
    id: &str,
    secret: &str,
) -> Result<(reqwest::Client, Token), Error> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let token = client
        .post(format!("{base}/v1/oauth2/token"))
        .basic_auth(id, Some(secret))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body("grant_type=client_credentials")
        .send()
        .await?
        .error_for_status()?
        .json::<Token>()
        .await?;
    Ok((client, token))
}

async fn fetch_balances(base: &str, mode: &str, id: &str, secret: &str) -> Result<String, Error> {
    let (client, token) = authenticate(base, id, secret).await?;
    let report = client
        .get(format!("{base}/v1/reporting/balances"))
        .bearer_auth(token.access_token)
        .send()
        .await?
        .error_for_status()?
        .json::<Report>()
        .await?;
    let mut lines = vec![format!("PayPal {mode}")];
    if let Some(time) = report.last_refresh_time {
        lines.push(format!("PayPal cập nhật lúc {time}"));
    }
    if report.balances.is_empty() {
        lines.push("PayPal chưa báo cáo số dư.".into());
    }
    for balance in report.balances {
        let total = balance.total_balance;
        let mut line = format!("{} {} tổng cộng", total.value, total.currency_code);
        if let Some(money) = balance.available_balance {
            line.push_str(&format!(
                " · {} {} khả dụng",
                money.value, money.currency_code
            ));
        }
        lines.push(line);
    }
    Ok(lines.join("\n"))
}

#[derive(Deserialize, JsonSchema)]
struct Period {
    /// First UTC calendar date, YYYY-MM-DD (inclusive).
    start_date: String,
    /// Last UTC calendar date, YYYY-MM-DD (inclusive, at most 31 days total).
    end_date: String,
    /// Page number starting at one. Each page has up to 100 transactions.
    page: Option<u32>,
}

async fn transactions(call: Call, period: Period) -> Result<String, Error> {
    let date = |s: &str| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d");
    let start = date(&period.start_date)?;
    let end = date(&period.end_date)?;
    let page = period.page.unwrap_or(1);
    if !(0..31).contains(&(end - start).num_days()) || page == 0 {
        return Err("Choose 1–31 calendar days and a positive page".into());
    }
    let (base, mode, id, secret) = credentials(&call)?;
    let (client, token) = authenticate(base, id, secret).await?;
    let mut url = reqwest::Url::parse(&format!("{base}/v1/reporting/transactions"))?;
    url.query_pairs_mut()
        .append_pair("start_date", &format!("{start}T00:00:00Z"))
        .append_pair("end_date", &format!("{end}T23:59:59Z"))
        .append_pair("fields", "transaction_info")
        .append_pair("page", &page.to_string())
        .append_pair("page_size", "100");
    let report: serde_json::Value = client
        .get(url)
        .bearer_auth(token.access_token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(serde_json::json!({"environment":mode,"page":page,"total_pages":report["total_pages"],"transactions":report["transaction_details"]}).to_string())
}

fn main() -> std::process::ExitCode {
    plugin_sdk::serve(
        Definition {
            tools: vec![
                Tool::handler("balances", Effect::Read,
                    balances,
                ),
                Tool::handler("list_transactions", Effect::Read,
                    transactions,
                ),
            ],
            ..Default::default()
        }
        .bind_manifest(include_str!("../manifest.json"))
        .expect("package manifest"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;

    #[tokio::test]
    async fn exchanges_credentials_and_reads_multiple_currencies() {
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", server.local_addr().unwrap());
        let handler = std::thread::spawn(move || {
            for (request, auth, body, response) in [
                (
                    "POST /v1/oauth2/token",
                    "Basic aWQ6c2VjcmV0",
                    "grant_type=client_credentials",
                    r#"{"access_token":"test-token"}"#,
                ),
                (
                    "GET /v1/reporting/balances",
                    "Bearer test-token",
                    "",
                    r#"{"balances":[{"total_balance":{"currency_code":"USD","value":"10.05"},"available_balance":{"currency_code":"USD","value":"8.01"}},{"total_balance":{"currency_code":"EUR","value":"0.00"}}]}"#,
                ),
            ] {
                let (mut socket, _) = server.accept().unwrap();
                let mut reader = BufReader::new(&socket);
                let mut headers = String::new();
                while !headers.ends_with("\r\n\r\n") {
                    assert_ne!(reader.read_line(&mut headers).unwrap(), 0);
                }
                assert!(headers.starts_with(request));
                assert!(headers.lines().any(|line| {
                    line.split_once(": ")
                        .is_some_and(|(k, v)| k.eq_ignore_ascii_case("authorization") && v == auth)
                }));
                let mut bytes = vec![0; body.len()];
                reader.read_exact(&mut bytes).unwrap();
                assert_eq!(bytes, body.as_bytes());
                write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",response.len(),response).unwrap();
            }
        });
        let text = fetch_balances(&base, "Sandbox", "id", "secret")
            .await
            .unwrap();
        handler.join().unwrap();
        assert_eq!(
            text,
            "PayPal Sandbox\n10.05 USD tổng cộng · 8.01 USD khả dụng\n0.00 EUR tổng cộng"
        );
    }
}
