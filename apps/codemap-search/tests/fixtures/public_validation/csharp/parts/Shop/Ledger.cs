namespace Parts;

public partial class Ledger
{
    public decimal Post(decimal amount) => Round(amount);
}
