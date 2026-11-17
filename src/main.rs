use multipart_commerce_rust::{advance_order, InfraiClient, Order, OrderState, ServiceError};
use serde_json::{json, Value};

fn field(data: &Value, name: &str) -> Result<String, ServiceError> {
    data[name]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| ServiceError::Api(format!("missing {name}: {data}")))
}

fn run() -> Result<(), ServiceError> {
    let client = InfraiClient::from_env()?;
    let name = format!(
        "commerce-media-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );
    let key = "orders/ord-1001/product-video.bin";
    let mut bucket = None;
    let mut upload = None;
    let mut completed = false;
    let result = (|| {
        let created = client.execute(client.bucket_create_request(&name))?;
        println!("created bucket: {created}");
        let bucket_id = created["bucket_id"]
            .as_str()
            .or_else(|| created["id"].as_str())
            .unwrap_or(&name)
            .to_owned();
        bucket = Some(bucket_id.clone());
        let data = client.execute(client.multipart_create_request(&bucket_id, key))?;
        let upload_id = field(&data, "upload_id")?;
        upload = Some(upload_id.clone());
        let signed = client.execute(client.presign_part_request(&upload_id, 1))?;
        let url = field(&signed, "url")?;
        let method = signed["method"].as_str().unwrap_or("PUT");
        let http = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .map_err(|e| ServiceError::Api(e.to_string()))?;
        let mut request = http.request(
            method
                .parse()
                .map_err(|e| ServiceError::Api(format!("{e}")))?,
            url,
        );
        if let Some(headers) = signed["headers"].as_object() {
            for (name, value) in headers {
                request = request.header(
                    name,
                    value
                        .as_str()
                        .ok_or_else(|| ServiceError::Api("invalid signed header".into()))?,
                );
            }
        }
        let response = request
            .body(b"product video demo".to_vec())
            .send()
            .and_then(reqwest::blocking::Response::error_for_status)
            .map_err(|e| ServiceError::Api(e.to_string()))?;
        let etag = response
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| ServiceError::Api("uploaded part has no ETag".into()))?
            .to_owned();
        let data = client.execute(
            client.complete_request(&upload_id, &[json!({"part_number": 1, "etag": etag})]),
        )?;
        completed = true;
        println!("completed upload: {data}");
        let mut order = Order {
            id: "ord-1001".into(),
            media_key: key.into(),
            state: OrderState::AwaitingUpload,
        };
        advance_order(&mut order, completed)?;
        println!("order {} is {:?}; receipt is ready", order.id, order.state);
        Ok(())
    })();
    // Cleanup runs even when an upload fails; propagate cleanup failures as well.
    let cleanup = (|| {
        if let Some(ref id) = upload {
            if !completed {
                client.delete(&format!("/v1/storage/multipart/abort/{id}"))?;
                println!("aborted upload {id}");
            }
        }
        if let Some(ref id) = bucket {
            let objects = client.list_objects(id)?;
            println!("objects before cleanup: {objects}");
            if completed {
                client.delete(&format!("/v1/storage/object/delete/{id}/{key}"))?;
                println!("objects after deletion: {}", client.list_objects(id)?);
            }
            client.delete(&format!("/v1/storage/bucket/delete/{id}"))?;
            match client.list_objects(id) {
                Err(ServiceError::Api(error))
                    if error.contains("404") || error.contains("NOT_FOUND") =>
                {
                    println!("verified bucket deleted: {id}");
                }
                other => {
                    return Err(ServiceError::Api(format!(
                        "bucket deletion not verified: {other:?}"
                    )))
                }
            }
        }
        Ok(())
    })();
    if let Err(error) = &result {
        eprintln!("workflow error: {error:?}");
    }
    cleanup?;
    result
}

fn main() {
    if let Err(error) = run() {
        eprintln!("service error: {error:?}");
        std::process::exit(1);
    }
}
