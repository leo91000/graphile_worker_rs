use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

pub(super) async fn write_http_response(
    stream: &mut TcpStream,
    status_code: u16,
    status_text: &str,
    body: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let response = format!(
        "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nContent-Type: text/plain\r\n\r\n{}",
        status_code,
        status_text,
        body.len(),
        body
    );
    stream.write_all(response.as_bytes()).await?;
    Ok(())
}
