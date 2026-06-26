use crate::audit::*;
use crate::repo::OrderRepo;

pub struct OrderLine {
    price: i64,
    quantity: i64,
}

pub struct OrderDto {
    total: i64,
}

pub trait Mapper {
    fn map(&self, lines: &[OrderLine]) -> OrderDto;
}

macro_rules! audit_event {
    ($name:expr, $dto:expr) => {
        trace_order($name, $dto.total)
    };
}

pub struct CheckoutService<M, I> {
    mapper: M,
    repo: OrderRepo,
    inventory: I,
}

impl<M, I> CheckoutService<M, I>
where
    M: Mapper,
    I: Inventory,
{
    pub fn submit(&self, lines: &[OrderLine]) -> Result<OrderDto, String> {
        let dto: OrderDto = self.mapper.map(lines);
        let audit = audit_event!("checkout.submit", dto);

        self.inventory.reserve(lines)?;
        self.repo.save(&dto)?;
        audit.finish();
        Ok(dto)
    }
}
