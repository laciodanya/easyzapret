import { Logo } from "./Logo";
import { Spinner } from "./ui";

export function BootSplash() {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-5">
      <Logo size={56} />
      <Spinner />
    </div>
  );
}
