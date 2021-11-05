use std::io::{BufReader, Error, Read, Result, Seek, SeekFrom};

use reqwest::{blocking::Client, IntoUrl, Url};

pub struct BufferedHttpReader(BufReader<HttpReader>, u64);
impl Read for BufferedHttpReader {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let sz = self.0.read(buf)?;
        self.1 += sz as u64;
        Ok(sz)
    }
}
impl Seek for BufferedHttpReader {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        let size = self.0.get_ref().size;
        let cursor = self.1;
        let diff = match pos {
            SeekFrom::Current(diff) => diff,
            SeekFrom::End(diff) => size as i64 + diff - cursor as i64,
            SeekFrom::Start(pos) => pos as i64 - cursor as i64,
        };
        self.0.seek_relative(diff)?;
        if diff > 0 {
            self.1 += diff as u64;
        } else {
            self.1 -= (-diff) as u64;
        }
        Ok(self.1)
    }
}

pub struct HttpReader {
    client: Client,
    url: Url,
    size: usize,
    cursor: usize,
}

fn map_result<T, E: std::error::Error + Sync + Send + 'static>(
    input: std::result::Result<T, E>,
) -> Result<T> {
    input.map_err(|e| Error::new(std::io::ErrorKind::Other, e))
}

impl HttpReader {
    pub fn buffered(mut self) -> Result<BufferedHttpReader> {
        self.seek(SeekFrom::Start(0))?;
        let br = BufReader::with_capacity(8192, self);
        Ok(BufferedHttpReader(br, 0))
    }
    pub fn new<U: IntoUrl>(url: U) -> Result<Self> {
        let url = map_result(url.into_url())?;
        let client = Client::new();

        let size: usize = {
            let response = map_result(
                client
                    .head(url.clone())
                    .header("connection", "keep-alive")
                    .send(),
            )?;
            let size_text = response.headers()["content-length"].as_bytes();
            let size_text = String::from_utf8_lossy(size_text);
            map_result(size_text.parse())?
        };

        Ok(Self {
            client,
            url,
            size,
            cursor: 0,
        })
    }
}

impl Read for HttpReader {
    fn read(&mut self, mut buf: &mut [u8]) -> Result<usize> {
        let to = self.size.min(self.cursor + buf.len() - 1);
        if self.cursor > to {
            return Ok(0);
        }
        let mut request = map_result(
            self.client
                .get(self.url.as_ref())
                .header("range", format!("bytes={}-{}", self.cursor, to))
                .header("connection", "keep-alive")
                .send(),
        )?;
        let mut sz = 0;
        while !buf.is_empty() {
            let read = request.read(buf)?;
            if read == 0 {
                break;
            }
            sz += read;
            buf = &mut buf[read..];
        }
        self.cursor += sz;
        Ok(sz)
    }
}

impl Seek for HttpReader {
    fn seek(&mut self, pos: std::io::SeekFrom) -> Result<u64> {
        let delta = match pos {
            SeekFrom::Current(delta) => delta,
            SeekFrom::End(delta) => {
                self.cursor = self.size;
                delta
            }
            SeekFrom::Start(delta) => {
                self.cursor = 0;
                delta as i64
            }
        };
        if delta > 0 {
            self.cursor += delta as usize;
            self.cursor = self.cursor.min(self.size);
        } else {
            if self.cursor < (-delta) as usize {
                self.cursor = 0;
            } else {
                self.cursor -= (-delta) as usize;
            }
        }
        Ok(self.cursor as u64)
    }
}
