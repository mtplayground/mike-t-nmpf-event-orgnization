#![allow(dead_code)]

use std::{fmt, time::Duration};

use aws_config::{BehaviorVersion, Region};
use aws_credential_types::Credentials;
use aws_sdk_s3::{
    Client,
    config::Builder as S3ConfigBuilder,
    error::SdkError,
    presigning::PresigningConfig,
};
use tracing::info;

use crate::config::ObjectStorageConfig;

#[derive(Clone)]
pub struct ObjectStorageClient {
    bucket: String,
    client: Client,
    public_base_url: Option<String>,
}

impl ObjectStorageClient {
    pub async fn from_config(config: &ObjectStorageConfig) -> Result<Self, ObjectStorageError> {
        let shared_config = aws_config::defaults(BehaviorVersion::latest())
            .region(Region::new(config.region.clone()))
            .credentials_provider(Credentials::new(
                config.access_key_id.clone(),
                config.secret_access_key.clone(),
                None,
                None,
                "event-organization-api",
            ))
            .load()
            .await;

        let s3_config = S3ConfigBuilder::from(&shared_config)
            .endpoint_url(config.endpoint.clone())
            .force_path_style(true)
            .build();

        info!(
            bucket = %config.bucket,
            endpoint = %config.endpoint,
            region = %config.region,
            "configured object storage client"
        );

        Ok(Self {
            bucket: config.bucket.clone(),
            client: Client::from_conf(s3_config),
            public_base_url: config.public_base_url.clone(),
        })
    }

    pub async fn put_presigned_url(
        &self,
        key: &str,
        content_type: Option<&str>,
        expires_in: Duration,
    ) -> Result<PresignedRequest, ObjectStorageError> {
        let mut request = self.client.put_object().bucket(&self.bucket).key(key);

        if let Some(content_type) = content_type {
            request = request.content_type(content_type);
        }

        let presigned_request = request
            .presigned(presigning_config(expires_in)?)
            .await
            .map_err(|error| ObjectStorageError::Presign(error.to_string()))?;

        Ok(PresignedRequest::from_aws_request(
            presigned_request,
            expires_in,
        ))
    }

    pub async fn get_presigned_url(
        &self,
        key: &str,
        expires_in: Duration,
    ) -> Result<PresignedRequest, ObjectStorageError> {
        let presigned_request = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .presigned(presigning_config(expires_in)?)
            .await
            .map_err(|error| ObjectStorageError::Presign(error.to_string()))?;

        Ok(PresignedRequest::from_aws_request(
            presigned_request,
            expires_in,
        ))
    }

    pub async fn delete_object(&self, key: &str) -> Result<(), ObjectStorageError> {
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|error| ObjectStorageError::DeleteObject(error.to_string()))?;

        Ok(())
    }

    pub async fn head_object(
        &self,
        key: &str,
    ) -> Result<Option<ObjectMetadata>, ObjectStorageError> {
        let response = self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await;

        match response {
            Ok(output) => Ok(Some(ObjectMetadata {
                key: key.to_owned(),
                bucket: self.bucket.clone(),
                content_length: output.content_length(),
                content_type: output.content_type().map(ToOwned::to_owned),
                e_tag: output.e_tag().map(ToOwned::to_owned),
                last_modified: output.last_modified().map(|value| value.to_string()),
                public_url: self.public_url_for(key),
            })),
            Err(SdkError::ServiceError(service_error))
                if service_error.err().is_not_found()
                    || service_error.raw().status().as_u16() == 404 =>
            {
                Ok(None)
            }
            Err(error) => Err(ObjectStorageError::HeadObject(error.to_string())),
        }
    }

    fn public_url_for(&self, key: &str) -> Option<String> {
        self.public_base_url
            .as_ref()
            .map(|base| format!("{}/{}", base.trim_end_matches('/'), key))
    }
}

#[derive(Debug, Clone)]
pub struct PresignedRequest {
    pub method: String,
    pub uri: String,
    pub headers: Vec<(String, String)>,
    pub expires_in: Duration,
}

impl PresignedRequest {
    fn from_aws_request(
        request: aws_sdk_s3::presigning::PresignedRequest,
        expires_in: Duration,
    ) -> Self {
        let headers = request
            .headers()
            .map(|(name, value)| (name.to_owned(), value.to_owned()))
            .collect();

        Self {
            method: request.method().to_string(),
            uri: request.uri().to_string(),
            headers,
            expires_in,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ObjectMetadata {
    pub key: String,
    pub bucket: String,
    pub content_length: Option<i64>,
    pub content_type: Option<String>,
    pub e_tag: Option<String>,
    pub last_modified: Option<String>,
    pub public_url: Option<String>,
}

#[derive(Debug)]
pub enum ObjectStorageError {
    InvalidPresignExpiry(String),
    Presign(String),
    DeleteObject(String),
    HeadObject(String),
}

impl fmt::Display for ObjectStorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPresignExpiry(message) => formatter.write_str(message),
            Self::Presign(error) => write!(formatter, "failed to presign object storage request: {error}"),
            Self::DeleteObject(error) => write!(formatter, "failed to delete object from storage: {error}"),
            Self::HeadObject(error) => write!(formatter, "failed to read object metadata from storage: {error}"),
        }
    }
}

impl std::error::Error for ObjectStorageError {}

fn presigning_config(expires_in: Duration) -> Result<PresigningConfig, ObjectStorageError> {
    PresigningConfig::expires_in(expires_in).map_err(|error| {
        ObjectStorageError::InvalidPresignExpiry(format!(
            "invalid presign expiration window: {error}"
        ))
    })
}
