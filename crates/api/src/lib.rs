use worker::*;
mod auth;
mod config;
mod mock;
mod store;
mod tracker_do;
mod upstream;
pub use tracker_do::Tracker;

#[event(fetch)]
pub async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    if !req.path().starts_with("/api/") {
        return Response::error("Not found", 404);
    }
    let stub = env
        .durable_object("TRACKER")?
        .id_from_name("tracker")?
        .get_stub()?;
    stub.fetch_with_request(req).await
}
