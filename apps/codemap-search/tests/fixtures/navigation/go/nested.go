package checkout

import "context"

type OrderLine struct {
	Price    int
	Quantity int
}

type OrderDTO struct {
	Total int
}

type Mapper interface {
	MapOrder([]OrderLine) OrderDTO
}

type Repository interface {
	Save(context.Context, OrderDTO) error
}

type Inventory interface {
	Reserve(context.Context, []OrderLine) error
}

type Service struct {
	mapper    Mapper
	repo      Repository
	inventory Inventory
}

func (s *Service) Submit(ctx context.Context, lines []OrderLine) error {
	dto := s.mapper.MapOrder(lines)
	audit := Trace(ctx, "checkout.submit")

	if err := s.inventory.Reserve(ctx, lines); err != nil {
		return err
	}
	if err := s.repo.Save(ctx, dto); err != nil {
		return err
	}
	audit.Done()
	return nil
}
