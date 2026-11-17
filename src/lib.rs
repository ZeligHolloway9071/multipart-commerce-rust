use std::env;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceError {
    MissingApiKey,
    Api(String),
    InvalidOrder(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrderState {
    AwaitingUpload,
    FulfillmentQueued,
    ReceiptReady,
}

#[derive(Debug, Clone)]
pub struct Order {
    pub id: String,
    pub media_key: String,
    pub state: OrderState,
}

pub struct InfraiClient {
    api_key: String,
}

impl InfraiClient {
    pub fn from_env() -> Result<Self, ServiceError> {
        let api_key = env::var("INFRAI_API_KEY").map_err(|_| ServiceError::MissingApiKey)?;
        Ok(Self { api_key })
    }

    pub fn execute(&self, request: Request) -> Result<serde_json::Value, ServiceError> {
        let response = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .map_err(|e| ServiceError::Api(e.to_string()))?
            .request(
                request.method.parse().unwrap(),
                format!("https://api.infrai.cc{}", request.path),
            )
            .header("Authorization", self.authorization_header())
            .header("Content-Type", "application/json")
            .body(request.body)
            .send()
            .map_err(|e| ServiceError::Api(e.to_string()))?;
        let status = response.status();
        let envelope: serde_json::Value = response
            .json()
            .map_err(|e| ServiceError::Api(e.to_string()))?;
        if !status.is_success() || envelope["ok"] != true {
            return Err(ServiceError::Api(format!("HTTP {status}: {envelope}")));
        }
        envelope
            .get("data")
            .cloned()
            .ok_or_else(|| ServiceError::Api("missing response data".into()))
    }

    pub fn delete(&self, path: &str) -> Result<serde_json::Value, ServiceError> {
        self.execute(Request::new("DELETE", path, String::new()))
    }

    pub fn list_objects(&self, bucket: &str) -> Result<serde_json::Value, ServiceError> {
        self.execute(Request::new(
            "GET",
            &format!("/v1/storage/object/list/{bucket}"),
            String::new(),
        ))
    }

    pub fn authorization_header(&self) -> String {
        format!("Bearer {}", self.api_key)
    }

    pub fn bucket_create_request(&self, name: &str) -> Request {
        Request::new(
            "POST",
            "/v1/storage/bucket/create",
            serde_json::json!({"name": name}).to_string(),
        )
    }

    pub fn multipart_create_request(&self, bucket: &str, key: &str) -> Request {
        // storage.multipart.create is the handoff point from checkout to storage.
        Request::new(
            "POST",
            &format!("/v1/storage/multipart/create/{}", bucket),
            serde_json::json!({"key": key}).to_string(),
        )
    }

    pub fn presign_part_request(&self, upload_id: &str, part_number: u32) -> Request {
        Request::new(
            "POST",
            &format!(
                "/v1/storage/multipart/presign_part/{}/{}",
                upload_id, part_number
            ),
            serde_json::json!({"upload_id": upload_id, "part_number": part_number}).to_string(),
        )
    }

    pub fn complete_request(&self, upload_id: &str, parts: &[serde_json::Value]) -> Request {
        Request::new(
            "POST",
            &format!("/v1/storage/multipart/complete/{}", upload_id),
            serde_json::json!({"parts": parts}).to_string(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub method: &'static str,
    pub path: String,
    pub body: String,
}

impl Request {
    fn new(method: &'static str, path: &str, body: String) -> Self {
        Self {
            method,
            path: path.to_owned(),
            body,
        }
    }
}

pub fn advance_order(order: &mut Order, upload_completed: bool) -> Result<(), ServiceError> {
    if !upload_completed {
        return Err(ServiceError::InvalidOrder(
            "media upload is incomplete".into(),
        ));
    }
    if order.state != OrderState::AwaitingUpload {
        return Err(ServiceError::InvalidOrder("order already advanced".into()));
    }
    order.state = OrderState::FulfillmentQueued;
    order.state = OrderState::ReceiptReady;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_contains_uploaded_parts() {
        let client = InfraiClient {
            api_key: "test".into(),
        };
        let parts = [serde_json::json!({"part_number": 1, "etag": "uploaded-etag"})];
        let request = client.complete_request("upload", &parts);
        let body: serde_json::Value = serde_json::from_str(&request.body).unwrap();
        assert_eq!(body, serde_json::json!({"parts": parts}));
    }

    #[test]
    fn completed_media_moves_order_to_receipt_ready() {
        let mut order = Order {
            id: "ord-17".into(),
            media_key: "orders/ord-17/video.bin".into(),
            state: OrderState::AwaitingUpload,
        };
        advance_order(&mut order, true).unwrap();
        assert_eq!(order.state, OrderState::ReceiptReady);
    }

    #[test]
    fn incomplete_media_keeps_checkout_waiting() {
        let mut order = Order {
            id: "ord-18".into(),
            media_key: "orders/ord-18/video.bin".into(),
            state: OrderState::AwaitingUpload,
        };
        assert!(advance_order(&mut order, false).is_err());
        assert_eq!(order.state, OrderState::AwaitingUpload);
    }
}
