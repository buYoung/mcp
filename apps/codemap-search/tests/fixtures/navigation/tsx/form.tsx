import { useState } from "react";

type Props = {
  service: CheckoutService;
  validator: OrderValidator;
};

export function CheckoutForm(props: Props) {
  const [errors, setErrors] = useState<string[]>([]);

  async function handleSubmit(input: OrderInput) {
    const nextErrors = props.validator.validate(input);
    if (nextErrors.length > 0) {
      setErrors(nextErrors);
      return;
    }

    await props.service.submit(input);
  }

  return <button onClick={() => handleSubmit({})}>{errors.length}</button>;
}
