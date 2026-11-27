namespace Parts;

// Another part of the class, in another folder of the same project.
public partial class Checkout
{
    private void Verify(decimal amount)
    {
        if (amount < 0)
        {
            throw new System.ArgumentOutOfRangeException(nameof(amount));
        }
    }
}
