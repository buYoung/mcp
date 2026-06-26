use crate::http::Response;

pub struct CheckoutHandler<S> {
    service: S,
}

impl<S> CheckoutHandler<S>
where
    S: CheckoutPort,
{
    pub async fn post(&self, body: Bytes) -> Result<Response, Error> {
        let payload = parse_order(body)?;
        let receipt = self.service.submit(payload).await?;
        Ok(Response::ok(receipt))
    }
}
