use warp::Filter;
use sqlx::{Pool, Postgres};
use crate::handlers::{register_user, UploadRequest, upload_file, list_all_nodes, RegisterRequest, download_file, NodeRegisterRequest, register_node, with_auth, heartbeat_node};
use uuid::Uuid;

pub fn make_routes(pool: Pool<Postgres>) -> impl Filter<Extract=impl warp::Reply, Error=warp::Rejection> + Clone {
    let pool_filter = warp::any().map(move || pool.clone());

    let register = warp::path("register")
        .and(warp::post())
        .and(warp::body::json())
        .and(pool_filter.clone())
        .and_then(|body: RegisterRequest, pool: Pool<Postgres>| async move {
            register_user(pool, body).await
        });

    let upload = warp::path("upload")
        .and(warp::post())
        .and(with_auth()) // 需要鉴权
        .and(warp::body::json())
        .and(pool_filter.clone())
        .and_then(|_, body: UploadRequest, pool: Pool<Postgres>| async move {
            upload_file(pool, body).await
        });

    let nodes = warp::path("nodes")
        .and(warp::get())
        .and(pool_filter.clone())
        .and_then(|pool: Pool<Postgres>| async move {
            list_all_nodes(pool).await
        });

    let download = warp::path("download")
        .and(warp::path::param::<Uuid>())
        .and(with_auth()) // 需要鉴权，增加一个 ()
        .and(warp::get())
        .and(pool_filter.clone())
        .and_then(|file_id: Uuid, _: (), pool: Pool<Postgres>| async move {
            download_file(pool, file_id).await
        });


    let register_node_route = warp::path("register_node")
        .and(warp::post())
        .and(warp::body::json())
        .and(pool_filter.clone())
        .and_then(|body: NodeRegisterRequest, pool: Pool<Postgres>| async move {
            register_node(pool, body).await
        });

    // 添加 heartbeat_node 到路由树
    let heartbeat_node = warp::path("heartbeat_node")
        .and(warp::post())
        .and(warp::body::json())
        .and(pool_filter.clone())
        .and_then(|node_id: Uuid, pool: Pool<Postgres>| async move {
            heartbeat_node(pool, node_id).await
        });

    register
        .or(upload)
        .or(nodes)
        .or(download)
        .or(register_node_route)
        .or(heartbeat_node) // 添加到最终返回的路由过滤器中
}
