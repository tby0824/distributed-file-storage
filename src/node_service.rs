use warp::Filter;
use std::fs::read;
use warp::http::StatusCode;

#[tokio::main]
async fn main() {
    let get_chunk = warp::path!("get_chunk" / String)
        .and(warp::get())
        .map(|chunk_id: String| {
            let chunk_path = format!("chunks/{}.bin", chunk_id);
            match read(&chunk_path) {
                Ok(data) => warp::reply::with_status(data, StatusCode::OK),
                Err(_) => warp::reply::with_status(
                    "Chunk not found".as_bytes().to_vec(),
                    StatusCode::NOT_FOUND,
                ),
            }
        });

    // 9000
    let addr = ([127, 0, 0, 1], 9000);
    println!("Node service running at http://127.0.0.1:9000");
    warp::serve(get_chunk).run(addr).await;
}
