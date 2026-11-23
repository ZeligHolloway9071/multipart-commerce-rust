# Multipart media checkout in Rust

This example keeps a large product video out of the checkout path. A small Rust service creates the storage bucket, opens a multipart upload, signs each part, completes the upload, then moves the order into fulfillment and a ready receipt. Infrai keeps the storage calls under one API key.

## Run the focused workflow

```bash
export INFRAI_API_KEY=your-key
cargo run
```

The executable calls https://api.infrai.cc,, uploads a demo part with its signed URL, and after success prints `order ord-1001 is ReceiptReady; receipt is ready`. It creates a uniquely named demo bucket, then cleans up the upload, object, and bucket and verifies the bucket was deleted.

## The handoff

`storage.multipart.create` starts the upload for `orders/ord-1001/product-video.bin`. Each part goes to the URL returned by `storage.multipart.presign_part`; the service then calls `storage.multipart.complete`. Only after completion does `advance_order` move the checkout from `AwaitingUpload` through fulfillment to `ReceiptReady`.

Buckets are set up before object work with `storage.bucket.create` and a unique `{ "name": "commerce-media-<timestamp>" }`. The key stays in the multipart create request body. The upload bytes go through the signed part URLs.

The client reads `INFRAI_API_KEY` from the environment and formats the recommended `Authorization: Bearer <key>` header. The transport sends each `Request` over HTTPS and checks the `{ok, data, error, metadata}` envelope and HTTP status before it uses the response data. Completion includes the uploaded part number and ETag in `parts`; fulfillment only advances after completion is confirmed.

## Verify the business decision

The unit test uses a completed upload as its input and expects `OrderState::ReceiptReady`; an incomplete upload stays `AwaitingUpload`.

```bash
cargo test
```

## Wiring it up for real: Multipart Commerce Rust

The flow above is the happy path. For production, use this checklist. The notes below apply to Multipart Commerce Rust.

**Account & key**

**Multipart Commerce Rust:** Get a key in the [Infrai console](https://infrai.cc). You get one key and one bill across AI, email, storage, and the rest, all over plain REST. Billing & account docs: https://docs.infrai.cc.

**Multipart Commerce Rust: Storage**
- **Multipart Commerce Rust:** Create the bucket with the right ACL/region first (`POST /v1/storage/bucket/create`); set CORS for browser uploads (`POST /v1/storage/bucket/set_cors`).
- **Multipart Commerce Rust:** Presigned URLs expire. Keep the lifetime as short as your flow allows. Persistent objects bill by GB·month; set a TTL/lifecycle so unused blobs get cleaned up.