use super::*;
use crate::sexpr::Sexp;
use crate::sexpr::{Error, ErrorKind};

pub(super) struct ResourceEncoder {
    maximum_work_bytes: usize,
    work_bytes: usize,
    maximum_encoded_bytes: usize,
    encoded_bytes: usize,
}

impl ResourceEncoder {
    pub(super) fn new(maximum_work_bytes: usize, maximum_encoded_bytes: usize) -> Self {
        Self {
            maximum_work_bytes,
            work_bytes: 0,
            maximum_encoded_bytes,
            encoded_bytes: 0,
        }
    }

    pub(super) fn files(&mut self, files: &[AuthoredEmbeddedFile]) -> Result<Sexp, Error> {
        let mut children = Vec::with_capacity(files.len().saturating_add(1));
        children.push(Sexp::Atom("embedded_files".to_owned()));
        for file in files {
            children.push(self.file(file)?);
        }
        Ok(Sexp::List(children))
    }

    fn file(&mut self, file: &AuthoredEmbeddedFile) -> Result<Sexp, Error> {
        let mut children = vec![
            Sexp::Atom("file".to_owned()),
            form("name", [Sexp::Quoted(file.name.clone())]),
            form("type", [Sexp::Atom(file.file_type.clone())]),
        ];
        match &file.data {
            AuthoredResourceData::DeclarationOnly => {}
            AuthoredResourceData::Empty => children.push(form("data", [])),
            AuthoredResourceData::Bytes(bytes) => {
                let work_bytes = encoded_work_bound(bytes.len())?;
                self.work_bytes = self
                    .work_bytes
                    .checked_add(work_bytes)
                    .ok_or_else(resource_work_limit)?;
                if self.work_bytes > self.maximum_work_bytes {
                    return Err(resource_work_limit());
                }
                let (checksum, encoded) = encode_bytes(bytes)?;
                self.encoded_bytes = self
                    .encoded_bytes
                    .checked_add(encoded.len())
                    .ok_or_else(resource_limit)?;
                if self.encoded_bytes > self.maximum_encoded_bytes {
                    return Err(resource_limit());
                }
                children.push(form("data", [Sexp::Atom(format!("|{encoded}|"))]));
                children.push(form("checksum", [Sexp::Quoted(checksum)]));
            }
        }
        Ok(Sexp::List(children))
    }
}

#[cfg(feature = "embedded-resource-zstd")]
fn encoded_work_bound(input_bytes: usize) -> Result<usize, Error> {
    let compressed = zstd::zstd_safe::compress_bound(input_bytes);
    let base64 = compressed
        .checked_add(2)
        .map(|value| value / 3)
        .and_then(|value| value.checked_mul(4))
        .ok_or_else(resource_work_limit)?;
    compressed
        .checked_add(base64)
        .ok_or_else(resource_work_limit)
}

#[cfg(not(feature = "embedded-resource-zstd"))]
fn encoded_work_bound(_input_bytes: usize) -> Result<usize, Error> {
    Ok(0)
}

#[cfg(feature = "embedded-resource-zstd")]
fn encode_bytes(bytes: &[u8]) -> Result<(String, String), Error> {
    let compressed = zstd::stream::encode_all(bytes, 3).map_err(|error| {
        Error::build(
            ErrorKind::InvalidBuildValue,
            format!("could not zstd-compress embedded resource: {error}"),
        )
    })?;
    let checksum = crate::embedded_resource::mmh3_128(bytes, false);
    Ok((checksum, encode_base64(&compressed)))
}

#[cfg(not(feature = "embedded-resource-zstd"))]
fn encode_bytes(_bytes: &[u8]) -> Result<(String, String), Error> {
    Err(Error::build(
        ErrorKind::InvalidBuildValue,
        "embedded resource byte authoring requires embedded-resource-zstd",
    ))
}

#[cfg(feature = "embedded-resource-zstd")]
fn encode_base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let capacity = bytes.len().div_ceil(3).saturating_mul(4);
    let mut output = String::with_capacity(capacity);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied();
        let third = chunk.get(2).copied();
        output.push(char::from(ALPHABET[usize::from(first >> 2)]));
        output.push(char::from(
            ALPHABET[usize::from((first & 0x03) << 4 | second.unwrap_or_default() >> 4)],
        ));
        output.push(
            second
                .map(|second| {
                    char::from(
                        ALPHABET
                            [usize::from((second & 0x0f) << 2 | third.unwrap_or_default() >> 6)],
                    )
                })
                .unwrap_or('='),
        );
        output.push(
            third
                .map(|third| char::from(ALPHABET[usize::from(third & 0x3f)]))
                .unwrap_or('='),
        );
    }
    output
}

fn form(head: &str, children: impl IntoIterator<Item = Sexp>) -> Sexp {
    let mut values = vec![Sexp::Atom(head.to_owned())];
    values.extend(children);
    Sexp::List(values)
}

fn resource_limit() -> Error {
    Error::build(
        ErrorKind::ResourceLimit,
        "authored embedded resource encoding exceeds max_resource_encoded_bytes",
    )
}

fn resource_work_limit() -> Error {
    Error::build(
        ErrorKind::ResourceLimit,
        "authored embedded resource work exceeds max_resource_work_bytes",
    )
}
