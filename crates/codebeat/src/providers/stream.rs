use anyhow::{bail, Result};
use tokio::io::{AsyncBufRead, AsyncBufReadExt};
pub async fn next_line<R: AsyncBufRead + Unpin>(
    reader: &mut R,
    limit: usize,
) -> Result<Option<String>> {
    let mut output = Vec::new();
    loop {
        let bytes = reader.fill_buf().await?;
        if bytes.is_empty() {
            return if output.is_empty() {
                Ok(None)
            } else {
                Ok(Some(String::from_utf8_lossy(&output).into_owned()))
            };
        }
        let newline = bytes.iter().position(|b| *b == b'\n');
        let len = newline.map(|n| n + 1).unwrap_or(bytes.len());
        if output.len() + len > limit {
            bail!("PROCESS_FRAME_TOO_LARGE");
        }
        output.extend_from_slice(&bytes[..len]);
        reader.consume(len);
        if newline.is_some() {
            if output.last() == Some(&b'\n') {
                output.pop();
            }
            if output.last() == Some(&b'\r') {
                output.pop();
            }
            return Ok(Some(String::from_utf8_lossy(&output).into_owned()));
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn bounded_unicode_and_truncated_frame() {
        let input = "中文\nlast".as_bytes();
        let mut reader = tokio::io::BufReader::new(input);
        assert_eq!(
            next_line(&mut reader, 100).await.unwrap(),
            Some("中文".into())
        );
        assert_eq!(
            next_line(&mut reader, 100).await.unwrap(),
            Some("last".into())
        );
        assert!(next_line(&mut reader, 100).await.unwrap().is_none());
        let mut reader = tokio::io::BufReader::new(&b"0123456789012345"[..]);
        assert!(next_line(&mut reader, 8).await.is_err());
    }
}
