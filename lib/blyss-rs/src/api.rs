use bzip2_rs::DecoderReader;
use std::{collections::HashMap, io::Read};

use crate::error::Error;
use base64::{engine::general_purpose, Engine as _};
use reqwest::multipart::{Form, Part};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use spiral_rs::{
    client::Client,
    key_value::{extract_result_impl, row_from_key, varint_decode},
    params::Params,
    util::params_from_json_obj,
};

/// HTTP GET request to the given URL with the given API key.
pub(crate) async fn http_get_string(url: &str, api_key: &str) -> Result<String, Error> {
    let req = reqwest::Client::new().get(url).header("x-api-key", api_key);
    let res = req.send().await?.text().await?;
    Ok(res)
}

/// HTTP POST request with binary body to the given URL with the given API key.
pub(crate) async fn http_post_bytes(
    url: &str,
    api_key: &str,
    data: Vec<u8>,
) -> Result<Vec<u8>, Error> {
    let req = reqwest::Client::new()
        .post(url)
        .body(data)
        .header("Content-Type", "application/octet-stream")
        .header("x-api-key", api_key);
    let res = req.send().await?;
    let resp_body = res.bytes().await?;
    Ok(resp_body.to_vec())
}

/// HTTP POST request with string body to the given URL with the given API key.
pub(crate) async fn http_post_string(
    url: &str,
    api_key: &str,
    data: String,
) -> Result<String, Error> {
    let req = reqwest::Client::new()
        .post(url)
        .body(data)
        .header("x-api-key", api_key);
    let res = req.send().await?.text().await?;
    Ok(res)
}

/// HTTP POST request to the given URL with the given API key.
pub(crate) async fn http_post_form_data(
    url: &str,
    api_key: &str,
    data: Vec<u8>,
    fields: HashMap<String, String>,
) -> Result<Vec<u8>, Error> {
    let mut form_data = Form::new();
    for (key, value) in fields {
        form_data = form_data.text(key, value);
    }
    form_data = form_data.part("file", Part::bytes(data));

    let req = reqwest::Client::new()
        .post(url)
        .multipart(form_data)
        .header("x-api-key", api_key);
    let res = req.send().await?;
    let resp_body = res.bytes().await?;
    Ok(resp_body.to_vec())
}

/// Decompress the given data using bzip2.
fn decompress(data: &[u8]) -> Result<Vec<u8>, Error> {
    let mut decoder = DecoderReader::new(data);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed)?;
    Ok(decompressed)
}

/// Serialize a list of chunks into a single byte array using the following format:
/// - 8 bytes: number of chunks (u64 LE)
/// - for each chunk:
///   - 8 bytes: chunk length (u64 LE)
///   - (chunk data)
fn serialize_chunks(data: &[Vec<u8>]) -> Vec<u8> {
    let mut serialized = Vec::new();
    serialized.extend(u64::to_le_bytes(data.len() as u64).to_vec());
    for chunk in data {
        serialized.extend(u64::to_le_bytes(chunk.len() as u64).to_vec());
        serialized.extend(chunk);
    }
    serialized
}

/// Deserialize a list of chunks from a single byte array in the following format:
/// - 8 bytes: number of chunks (u64 LE)
/// - for each chunk:
///   - 8 bytes: chunk length (u64 LE)
///   - (chunk data)
fn deserialize_chunks(data: &[u8]) -> Vec<Vec<u8>> {
    let mut chunks = Vec::new();
    let mut offset = 0;
    let num_chunks = u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
    offset += 8;
    for _ in 0..num_chunks {
        let chunk_len = u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
        offset += 8;
        chunks.push(data[offset..offset + chunk_len as usize].to_vec());
        offset += chunk_len as usize;
    }
    chunks
}

/// Split the given data into metadata and the rest of the data.
fn split_metadata(data: &[u8]) -> (&[u8], &[u8]) {
    let (value, bytes_used) = varint_decode(data);
    let metadata_len = value as usize;
    if metadata_len == 0 {
        return (&[], data);
    }
    let metadata = &data[bytes_used..bytes_used + metadata_len];
    let data = &data[bytes_used + metadata_len..];

    (metadata, data)
}

/// Return whether the given data is all zeros.
fn is_all_zeros(decrypted: &[u8]) -> bool {
    decrypted.iter().all(|&x| x == 0)
}

/// Fetch the metadata from the given URL.
pub(crate) async fn get_meta(url: &str, api_key: &str) -> Result<String, Error> {
    http_get_string(&format!("{}/meta", url), api_key).await
}

fn is_blyss_url(url: &str) -> bool {
    url.contains("blyss.dev/")
}

#[derive(Serialize, Deserialize)]
struct PrelimSetupBody {
    length: usize,
}

async fn perform_setup(url: &str, api_key: &str, setup_data: Vec<u8>) -> Result<String, Error> {
    if !is_blyss_url(url) {
        let setup_resp = http_post_bytes(&format!("{}/setup", url), api_key, setup_data).await?;
        let setup_resp_str = String::from_utf8(setup_resp)?;
        let uuid = serde_json::from_str::<Value>(&setup_resp_str)?
            .get("uuid")
            .ok_or(Error::Unknown)?
            .as_str()
            .ok_or(Error::Unknown)?
            .to_string();
        return Ok(uuid);
    }

    let prelim_setup_body = serde_json::to_string(&PrelimSetupBody {
        length: setup_data.len(),
    })?;
    let setup_resp =
        http_post_string(&format!("{}/setup", url), api_key, prelim_setup_body).await?;
    let setup_resp_value: Value = serde_json::from_str(&setup_resp)?;
    let fields: HashMap<String, String> = serde_json::from_value(
        setup_resp_value
            .get("fields")
            .ok_or(Error::Unknown)?
            .clone(),
    )?;
    let s3_url: String =
        serde_json::from_value(setup_resp_value.get("url").ok_or(Error::Unknown)?.clone())?;

    http_post_form_data(&s3_url, api_key, setup_data, fields).await?;

    let uuid = setup_resp_value
        .get("uuid")
        .ok_or(Error::Unknown)?
        .as_str()
        .unwrap()
        .to_owned();
    Ok(uuid)
}

/// Privately read the given keys from the given URL, using the given API key.
async fn private_read<'a>(
    client: &Client<'a>,
    params: &Params,
    uuid: &str,
    url: &str,
    api_key: &str,
    keys: &[String],
) -> Result<Vec<Vec<u8>>, Error> {
    let queries: Vec<_> = keys
        .iter()
        .map(|key| {
            let idx_target = row_from_key(&params, key);
            let query = client.generate_query(idx_target);
            let query_data = query.serialize();
            let uuid_and_query_data: Vec<_> = (uuid.as_bytes().to_vec().into_iter())
                .chain(query_data)
                .collect();
            uuid_and_query_data
        })
        .collect();
    let full_query_data = serialize_chunks(&queries);

    let resp_data_b64 =
        http_post_bytes(&format!("{}/private-read", url), api_key, full_query_data).await?;
    let resp_data = general_purpose::STANDARD.decode(resp_data_b64).unwrap();
    let resp_chunks = deserialize_chunks(&resp_data);

    let mut results = Vec::new();
    for (i, chunk) in resp_chunks.iter().enumerate() {
        let decrypted = client.decode_response(&chunk);
        if is_all_zeros(&decrypted) {
            results.push(vec![]);
            continue;
        }
        let decompressed = decompress(&decrypted)?;
        let result = extract_result_impl(&keys[i], &decompressed);
        if let Ok(result) = result {
            let (_metadata, data) = split_metadata(&result);
            results.push(data.to_vec());
        } else {
            results.push(vec![]);
        }
    }

    Ok(results)
}

/// A client for a single, existing Blyss bucket.
pub struct ApiClient {
    /// The URL for the bucket.
    pub url: String,

    api_key: String,
    params: &'static Params,
    client: Client<'static>,
    uuid: Option<String>,
}

impl ApiClient {
    /// Create a new API client for the given URL and API key.
    ///
    /// The URL should be the URL of the bucket, e.g. `https://beta.api.blyss.dev/global.abc123`.
    pub async fn new(url: &str, api_key: &str) -> Result<Self, Error> {
        let metadata = get_meta(url, api_key).await?;
        let params_value = serde_json::from_str::<Value>(&metadata)
            .unwrap()
            .get("pir_scheme")
            .unwrap()
            .clone();
        let params = params_from_json_obj(&params_value);
        let boxed_params = Box::leak(Box::new(params)); // TODO: avoid this

        Ok(Self {
            url: url.to_string(),
            api_key: api_key.to_string(),
            params: boxed_params,
            client: Client::init(boxed_params),
            uuid: None,
        })
    }

    /// Returns whether the client has been set up for private reads.
    fn has_set_up(&self) -> bool {
        self.uuid.is_some()
    }

    /// Prepare the client for private reads. This must be called before calling private_read().
    pub async fn setup(&mut self) -> Result<(), Error> {
        let setup = self.client.generate_keys();
        let setup_data = setup.serialize();

        let uuid = perform_setup(&self.url, &self.api_key, setup_data).await?;

        self.uuid = Some(uuid);

        Ok(())
    }

    /// Privately read the given keys from the bucket.
    /// Must call setup() before calling this.
    ///
    /// # Arguments
    /// - `keys` - The keys to read.
    ///
    /// # Returns
    /// A vector of the values corresponding to the given keys.
    /// If a key does not exist, the corresponding value will be an empty vector.
    ///
    /// # Errors
    /// - `Error::NeedSetup` - If setup() has not been called.
    pub async fn private_read(&self, keys: &[String]) -> Result<Vec<Vec<u8>>, Error> {
        if !self.has_set_up() {
            return Err(Error::NeedSetup);
        }

        private_read(
            &self.client,
            &self.params,
            self.uuid.as_ref().unwrap(),
            &self.url,
            &self.api_key,
            keys,
        )
        .await
    }
}
