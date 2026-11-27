namespace Parts;

public partial class Checkout
{
    private readonly Ledger ledger = new();

    public decimal Close(decimal amount)
    {
        Verify(amount);
        return ledger.Post(amount);
    }
}
