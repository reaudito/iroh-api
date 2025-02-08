use axum::{
    extract::{Multipart, State},
    routing::{post, get},
    response::{IntoResponse, Json},
    Router,
};
use tower_http::cors::{Any, CorsLayer};
use std::fs;
use std::path::Path;
use serde::Serialize;
use anyhow::Result;
use iroh::{protocol::Router as IrohRouter, Endpoint, SecretKey};
use iroh_blobs::{
    net_protocol::Blobs,
    ticket::BlobTicket,
    util::local_pool::LocalPool,
};
use tokio::net::TcpListener;

#[derive(Clone)]
struct AppState {
    blobs: Blobs<iroh_blobs::store::fs::Store>,
    node_id: iroh::PublicKey,
}

#[derive(Serialize)]
struct UploadResponse {
    ticket: String,
    node_id: String,
    blob_hash: String,
    blob_format: String,
}

fn load_or_generate_secret_key(file_path: &str) -> SecretKey {
    let path = Path::new(file_path);
    if path.exists() {
        // Load the secret key from the file
        let key_bytes = fs::read(path).expect("Failed to read secret key file");

        // Ensure the key is exactly 32 bytes
        let key_array: [u8; 32] = key_bytes
            .try_into()
            .expect("Secret key file must be exactly 32 bytes");

        SecretKey::from_bytes(&key_array)
    } else {
        // Generate a new secret key and save it to the file
        let secret_key = SecretKey::generate(rand::rngs::OsRng);
        let key_bytes = secret_key.to_bytes();

        fs::write(path, &key_bytes).expect("Failed to write secret key file");
        secret_key
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize secret key, endpoint, blobs, and router

    let secret_key_path = "secret/secret_key.bin";
    let secret_key = load_or_generate_secret_key(secret_key_path);
    // let secret_key = SecretKey::from_bytes(&[
    //     7, 248, 9, 217, 34, 111, 158, 135, 199, 100, 110, 193, 1, 232, 53, 11, 121, 235, 201, 241,
    //     64, 188, 34, 219, 189, 167, 10, 134, 165, 2, 59, 254,
    // ]);
    let endpoint = Endpoint::builder()
        .secret_key(secret_key)
        .discovery_n0()
        .bind()
        .await?;

    let local_pool = LocalPool::default();
    let blobs = Blobs::persistent("data").await?.build(&local_pool, &endpoint);



    let node = IrohRouter::builder(endpoint)
        .accept(iroh_blobs::ALPN, blobs.clone())
        .spawn()
        .await?;

    let node_id  = node.endpoint().node_id();

    let app_state = AppState{
        blobs,
        node_id
    };

    let cors = CorsLayer::new()
        .allow_origin(Any) // Allow any origin (use a specific one in production)
        .allow_methods(Any)
        .allow_headers(Any);
    // Build Axum app
    let app = Router::new()
    .route("/upload", post(upload_file))
    .route("/node-id", get(get_node_id)) // New route for node ID
    .with_state(app_state).layer(cors);

    // Start the server
    let listener = TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await.unwrap();

    // Gracefully shut down the node
    node.shutdown().await?;
    local_pool.shutdown().await;
    Ok(())
}

async fn upload_file(
    State(app_state): State<AppState>, // Extract shared state
    mut multipart: Multipart,         // Extract multipart form data
) -> Result<impl IntoResponse, axum::http::StatusCode> {
    let blobs_client = app_state.blobs.client();

    while let Some(field) = multipart.next_field().await.unwrap() {
        let file_name = field.file_name().unwrap_or("unknown").to_string();
        let data = field.bytes().await.unwrap();

        // Attempt to add the bytes to the blob store
        let blob = blobs_client
            .add_bytes(data.clone())
            .await
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

        let node_id: iroh::PublicKey = app_state.node_id;

        // Attempt to generate the ticket
        let ticket = BlobTicket::new(node_id.into(), blob.hash, blob.format)
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

        println!("Received file: {} ({} bytes)", file_name, data.len());

        // Return the response with ticket, node_id, blob.hash, and blob.format
        return Ok(Json(UploadResponse {
            ticket: ticket.to_string(),
            node_id: node_id.to_string(),
            blob_hash: blob.hash.to_string(),
            blob_format: blob.format.to_string(),
        }));
    }

    // Return a bad request error if no file is uploaded
    Err(axum::http::StatusCode::BAD_REQUEST)
}


async fn get_node_id(State(app_state): State<AppState>) -> impl IntoResponse {
    Json(serde_json::json!({
        "node_id": app_state.node_id.to_string(),
    }))
}