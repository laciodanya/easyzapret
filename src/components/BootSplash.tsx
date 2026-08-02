import { Logo } from "./Logo";
import { Spinner } from "./ui";

export function BootSplash() {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-5">
      <div className="ez-fade-up">
        <Logo size={64} />
      </div>
      <div className="ez-fade-up-delay-1 flex items-center gap-2.5 text-sm text-[rgb(var(--text-secondary))]">
        <Spinner />
        <span className="font-medium tracking-tight">EasyZapret</span>
      </div>
    </div>
  );
}
