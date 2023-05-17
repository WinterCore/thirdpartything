use std::collections::HashMap;
use std::path::PathBuf;
use std::str;

#[derive(Debug)]
pub enum HttpVerb {
    Get,
    Post,
    Update,
    Put,
    Delete,
}

#[derive(Debug)]
pub enum HttpVersion {
    v0_9,
    v1_0,
    v1_1,
    v2_0,
    v3_0,
}

#[derive(Debug)]
pub struct HttpRequest {
    pub verb: HttpVerb,
    pub version: HttpVersion,
    pub pathname: PathBuf,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl HttpRequest {
    fn read_head(input: &[u8]) -> Result<&[u8], String> {
        let mut i = 0;
        let mut j = 0;
        let separator: Vec<u8> = "\r\n\r\n"
            .chars()
            .map(|x| x as u8)
            .collect();

        for byte in input {
            if j == 4 {
                return Ok(&input[0..i]);
            }

            if *byte == separator[j] {
                j += 1;
            } else {
                j = 0;
            }

            i += 1;
        }

        Err("Couldn't find header separator".to_owned())
    }

    fn parse_request_line(input: &str) -> Option<(HttpVerb, PathBuf, HttpVersion)> {
        let parts: Vec<&str> = input.split(" ").collect();

        let verb = match parts.get(0)?.to_ascii_uppercase().as_str() {
            "GET" => HttpVerb::Get,
            "POST" => HttpVerb::Post,
            "UPDATE" => HttpVerb::Update,
            "DELETE" => HttpVerb::Delete,
            _ => return None,
        };

        let pathname = PathBuf::from(parts.get(1)?);
        let version = match *parts.get(2)? {
            "HTTP/0.9" => HttpVersion::v0_9,
            "HTTP/1.0" => HttpVersion::v1_0,
            "HTTP/1.1" => HttpVersion::v1_1,
            "HTTP/2.0" => HttpVersion::v2_0,
            _ => return None,
        };

        Some((verb, pathname, version))
    }

    fn parse_headers(lines: Vec<&str>) -> HashMap<String, String> {
        let mut header_map = HashMap::<String, String>::new();

        lines.iter()
            .map(|x| x.trim())
            .filter(|x| x.len() > 0)
            .for_each(|x| {
                let header_parts = x.split_once(":");
                
                if let Some((key, value)) = header_parts {
                    header_map.insert(
                        key.trim().to_owned(),
                        value.trim().to_owned(),
                    );
                }
            });

        header_map
    }

    pub fn parse(input: &[u8]) -> Result<HttpRequest, String> {
        let head_bin = Self::read_head(input)?;
        let mut head = str::from_utf8(head_bin)
            .map_err(|_| "Failed to parse http head".to_owned())?
            .split("\r\n");

        let request_line = head.next().ok_or("Failed to parse http head".to_owned())?;
        let (verb, pathname, version) = Self::parse_request_line(request_line)
            .ok_or("Failed to parse http head".to_owned())?;
        let header_part = head.collect::<Vec<&str>>();

        let headers_map = Self::parse_headers(header_part);

        let body = input[head_bin.len()..].to_owned();
        
        Ok(Self {
            verb,
            version,
            pathname,
            headers: headers_map,
            body,
        })
    }
}
