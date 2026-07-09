import { Logo } from "./Logo";
import { Spinner } from "./ui";

export function BootSplash() {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-5 bg-[rgb(var(--surface))]">
      <Logo size={56} />
      <div className="flex items-center gap-2.5 text-sm text-[rgb(var(--text-secondary))]">
        <Spinner />
        <span>EasyZapret</span>
      </div>
    </div>
  );
}
