use async_std::sync::{Mutex, MutexGuard};
use diesel::r2d2::R2D2Connection;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};

use std::sync::Arc;
use tide::{utils::async_trait, Middleware, Next, Request};

#[derive(Clone)]
pub struct DieselMiddleware<C>
where
    C: R2D2Connection + 'static,
{
    pool: Pool<ConnectionManager<C>>,
}

impl<C> DieselMiddleware<C>
where
    C: R2D2Connection + 'static,
{
    pub async fn new(db_uri: &'_ str) -> std::result::Result<Self, Box<dyn std::error::Error>> {
        let manager = ConnectionManager::<C>::new(db_uri);
        let pg_conn = diesel::r2d2::Builder::<ConnectionManager<C>>::new()
            .build(manager)
            .map_err(|e| Box::new(e))?;
        Ok(Self { pool: pg_conn })
    }
}

impl<C> AsRef<Pool<ConnectionManager<C>>> for DieselMiddleware<C>
where
    C: R2D2Connection + 'static,
{
    fn as_ref(&self) -> &Pool<ConnectionManager<C>> {
        &self.pool
    }
}

impl<C> From<Pool<ConnectionManager<C>>> for DieselMiddleware<C>
where
    C: R2D2Connection + 'static,
{
    fn from(pool: Pool<ConnectionManager<C>>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl<State, C> Middleware<State> for DieselMiddleware<C>
where
    C: R2D2Connection + 'static,
    State: Clone + Send + Sync + 'static,
{
    async fn handle(&self, mut req: Request<State>, next: Next<'_, State>) -> tide::Result {
        if req
            .ext::<Arc<Mutex<Pool<ConnectionManager<C>>>>>()
            .is_some()
        {
            return Ok(next.run(req).await);
        }

        let conn: Arc<Pool<ConnectionManager<C>>> = Arc::new(self.pool.clone());
        req.set_ext(conn.clone());
        let res = next.run(req).await;
        Ok(res)
    }
}

#[async_trait]
pub trait DieselRequestExt<C: R2D2Connection + 'static> {
    async fn pg_conn(
        &self,
    ) -> Result<PooledConnection<ConnectionManager<C>>, diesel::r2d2::PoolError>;
    async fn pg_pool_conn(&self) -> MutexGuard<Pool<ConnectionManager<C>>>;
}

#[async_trait]
impl<T, C: R2D2Connection + 'static> DieselRequestExt<C> for Request<T>
where
    T: Send + Sync + 'static,
{
    async fn pg_conn(
        &self,
    ) -> Result<PooledConnection<ConnectionManager<C>>, diesel::r2d2::PoolError> {
        let pg_conn: &Arc<Pool<ConnectionManager<C>>> =
            self.ext().expect("You must install the Diesel middleware");
        pg_conn.get()
    }

    async fn pg_pool_conn(&self) -> MutexGuard<Pool<ConnectionManager<C>>> {
        let pg_conn: &Arc<Mutex<Pool<ConnectionManager<C>>>> =
            self.ext().expect("You must install the Diesel middleware");
        pg_conn.lock().await
    }
}
