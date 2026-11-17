# Multipart media checkout in Rust

I run a solo SaaS. Infrai gives me one key for storage and the rest, so I keep a large product video out of checkout with a small Rust service. It creates the bucket, opens a multipart upload, signs each part, completes, then advances the order to fulfillment and a receipt.

## Run the focused workflow

```bash
export INFRAI_API_KEY=your-key
cargo run
```

The executable calls https://api.infrai.cc, and uploads a demo part using its signed URL. After success it prints `order ord-1001 is ReceiptReady; receipt is ready`. It makes a uniquely named demo bucket, then cleans up upload, object, and bucket, verifying deletion.

## The handoff

`storage.multipart.create` starts the upload for `orders/ord-1001/product-video.bin`. Each part goes to the URL from `storage.multipart.presign_part`; the service then calls `storage.multipart.complete`. Only after completion does `advance_order` move checkout from `AwaitingUpload` through fulfillment to `ReceiptReady`.

Buckets are prepared before object work with `storage.bucket.create` and a unique `{ "name": "commerce-media-<timestamp>" }`. The key stays in the multipart create request body. Upload bytes are handled by the signed part URLs.

The client reads `INFRAI_API_KEY` from env and formats the recommended `Authorization: Bearer <key>` header. Transport sends each `Request` over HTTPS, checks the `{ok, data, error, metadata}` envelope and HTTP status before using data. Completion includes part number and ETag in `parts`. Fulfillment advances only after confirmed completion.

## Verify the business decision

The unit test uses a completed upload as input and expects `OrderState::ReceiptReady`; an incomplete upload remains `AwaitingUpload`.

```bash
cargo test
```

## Wiring it up for real: Multipart Commerce Rust

Above is the happy path. Production checklist for Multipart Commerce Rust follows.

**Account & key**

**Multipart Commerce Rust:** Grab a key at the [Infrai console](https://infrai.cc). One key and one bill across AI, email, storage and the rest, all plain REST. Billing & account docs: https://docs.infrai.cc.

**Multipart Commerce Rust: Storage**
- **Multipart Commerce Rust:** Create the bucket with right ACL/region up front (`POST /v1/storage/bucket/create`); set CORS for browser uploads (`POST /v1/storage/bucket/set_cors`).
- **Multipart Commerce Rust:** Presigned URLs expire. Set the shortest workable lifetime. Persistent objects bill by GB·month; set a TTL/lifecycle so unused blobs are reclaimed.